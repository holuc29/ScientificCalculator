//! A calculator session: expression evaluation plus memory, history,
//! variables, angle mode and display settings.

use std::collections::HashMap;

use crate::error::CalcError;
use crate::eval::{eval, is_reserved, Env};
use crate::fasteval::{self, PlotFn};
use crate::functions::Registry;
use crate::lexer::lex;
use crate::num::{AngleMode, Ctx, Num, DEFAULT_DISPLAY_DIGITS, MAX_DISPLAY_DIGITS};
use crate::parser::{parse, Expr};
use crate::programmer::{self, Outcome, WordSize};
use crate::symbolic::{self, Derivative};
use crate::userfn::{self, UserFn};

/// One successfully evaluated input.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub input: String,
    pub output: String,
    pub value: Num,
}

pub struct Calculator {
    ctx: Ctx,
    registry: Registry,
    vars: HashMap<String, Num>,
    user_fns: HashMap<String, UserFn>,
    memory: Option<Num>,
    ans: Num,
    history: Vec<HistoryEntry>,
    display_digits: usize,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    pub fn new() -> Self {
        Calculator {
            ctx: Ctx::new(),
            registry: Registry::with_builtins(),
            vars: HashMap::new(),
            user_fns: HashMap::new(),
            memory: None,
            ans: Num::zero(),
            history: Vec::new(),
            display_digits: DEFAULT_DISPLAY_DIGITS,
        }
    }

    // ------------------------------------------------------------- evaluation

    /// Evaluates one statement (an expression or `name = expression`).
    /// On success the result becomes `ans` and is appended to the history.
    pub fn eval(&mut self, input: &str) -> Result<Num, CalcError> {
        let expr = self.parse_statement(input)?;

        let (target, body) = match &expr {
            Expr::Assign(name, rhs) => {
                if is_reserved(name) || self.registry.contains(name) || self.user_fns.contains_key(name) {
                    return Err(CalcError::Assignment(format!("'{name}' is a reserved name")));
                }
                (Some(name.clone()), &**rhs)
            }
            other => (None, other),
        };

        let value = {
            let mut env = Env {
                ctx: &mut self.ctx,
                registry: &self.registry,
                vars: &self.vars,
                memory: self.memory.as_ref(),
                ans: &self.ans,
            };
            eval(body, &mut env)?
        };

        if let Some(name) = target {
            self.vars.insert(name, value.clone());
        }
        self.ans = value.clone();
        let output = self.format(&value);
        self.history.push(HistoryEntry {
            input: input.trim().to_string(),
            output,
            value: value.clone(),
        });
        Ok(value)
    }

    /// Evaluates and formats in one step.
    pub fn eval_str(&mut self, input: &str) -> Result<String, CalcError> {
        let v = self.eval(input)?;
        Ok(self.format(&v))
    }

    /// Renders a number using the current display precision.
    pub fn format(&mut self, n: &Num) -> String {
        self.ctx.format(n, self.display_digits)
    }

    /// Nearest `f64`, for plotting and similar lossy uses.
    pub fn to_f64(&mut self, n: &Num) -> Option<f64> {
        self.ctx.to_f64(n)
    }

    // --------------------------------------------------------------- settings

    pub fn angle_mode(&self) -> AngleMode {
        self.ctx.angle
    }

    pub fn set_angle_mode(&mut self, mode: AngleMode) {
        self.ctx.angle = mode;
    }

    pub fn display_digits(&self) -> usize {
        self.display_digits
    }

    /// Sets the number of significant digits shown (1 to 60).
    pub fn set_display_digits(&mut self, digits: usize) {
        self.display_digits = digits.clamp(1, MAX_DISPLAY_DIGITS);
    }

    // ----------------------------------------------------------------- memory

    pub fn memory(&self) -> Option<&Num> {
        self.memory.as_ref()
    }

    /// MC
    pub fn memory_clear(&mut self) {
        self.memory = None;
    }

    /// MS
    pub fn memory_store(&mut self, v: Num) {
        self.memory = Some(v);
    }

    /// M+
    pub fn memory_add(&mut self, v: &Num) -> Result<(), CalcError> {
        let cur = self.memory.clone().unwrap_or_else(Num::zero);
        self.memory = Some(self.ctx.add(&cur, v)?);
        Ok(())
    }

    /// M-
    pub fn memory_subtract(&mut self, v: &Num) -> Result<(), CalcError> {
        let cur = self.memory.clone().unwrap_or_else(Num::zero);
        self.memory = Some(self.ctx.sub(&cur, v)?);
        Ok(())
    }

    /// The last result (`ans`).
    pub fn last_answer(&self) -> &Num {
        &self.ans
    }

    // ---------------------------------------------------- history & variables

    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// User-defined variables, sorted by name.
    pub fn variables(&self) -> Vec<(&str, &Num)> {
        let mut v: Vec<_> = self.vars.iter().map(|(k, n)| (k.as_str(), n)).collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v
    }

    pub fn clear_variables(&mut self) {
        self.vars.clear();
    }

    /// Defines (or replaces) a variable from an expression. Unlike
    /// [`Calculator::eval`] with `name = expr`, this does not touch `ans` or the
    /// history. The name must be a plain identifier that is not reserved and not
    /// a function name.
    pub fn set_variable(&mut self, name: &str, input: &str) -> Result<Num, CalcError> {
        let name = name.trim().to_lowercase();
        let tokens = lex(&name)?;
        let single_name = tokens.len() == 2 && matches!(tokens[0].tok, crate::lexer::Tok::Ident(_));
        if !single_name {
            return Err(CalcError::Assignment(format!(
                "'{name}' is not a valid name (use letters, digits and _, starting with a letter)"
            )));
        }
        if is_reserved(&name) || self.registry.contains(&name) || self.user_fns.contains_key(&name) {
            return Err(CalcError::Assignment(format!("'{name}' is a reserved name")));
        }
        let value = self.eval_quiet(input)?;
        self.vars.insert(name, value.clone());
        Ok(value)
    }

    /// Forgets one variable; returns whether it existed.
    pub fn remove_variable(&mut self, name: &str) -> bool {
        self.vars.remove(&name.trim().to_lowercase()).is_some()
    }

    // -------------------------------------------------------------- programmer

    /// Evaluates one Programmer-mode statement (fixed-width integer
    /// arithmetic, see [`crate::programmer`]). Variables are shared with the
    /// scientific calculator but must be whole numbers that fit `word`;
    /// `name = expression` stores the result (as a signed value) in the shared
    /// variables. `ans` and the history are not touched.
    pub fn programmer_eval(&mut self, input: &str, word: WordSize) -> Result<Outcome, CalcError> {
        let vars = &self.vars;
        let ctx = &mut self.ctx;
        let mut lookup = |name: &str| -> Result<i128, CalcError> {
            if matches!(name, "pi" | "e") {
                return Err(CalcError::Domain(format!("'{name}' is not a whole number")));
            }
            let num = vars
                .get(name)
                .ok_or_else(|| CalcError::UnknownIdentifier(name.to_string()))?;
            let whole = ctx.as_integer(num).ok_or_else(|| {
                CalcError::Domain(format!(
                    "variable '{name}' is not a whole number ({})",
                    ctx.format(num, 12)
                ))
            })?;
            ctx.to_i128(&whole)
                .ok_or_else(|| CalcError::Domain(format!("variable '{name}' is too large")))
        };
        let (value, target) = programmer::run(input, word, &mut lookup)?;
        if let Some(name) = &target {
            if is_reserved(name)
                || self.registry.contains(name)
                || self.user_fns.contains_key(name)
                || matches!(name.as_str(), "rol" | "ror") {
                return Err(CalcError::Assignment(format!("'{name}' is a reserved name")));
            }
            self.vars.insert(name.clone(), Num::from_i64(value.signed()));
        }
        Ok(Outcome {
            value,
            assigned: target,
        })
    }

    // -------------------------------------------------------------- functions

    /// Lexes and parses one statement; user functions are known to the parser
    /// and their calls are inlined, so the result only contains built-ins.
    fn parse_statement(&self, input: &str) -> Result<Expr, CalcError> {
        let tokens = lex(input)?;
        if userfn::definition_head(&tokens).is_some() {
            return Err(CalcError::Syntax(
                "a function definition cannot be used as a value".into(),
            ));
        }
        let (registry, fns) = (&self.registry, &self.user_fns);
        let expr = parse(&tokens, &|n| registry.contains(n) || fns.contains_key(n))?;
        userfn::inline(&expr, fns)
    }

    /// If `input` has the form `name(p1, p2, ...) = body`, stores the function
    /// and returns its display text (`Ok(Some("f(x) = x^2"))`). Returns
    /// `Ok(None)` when the input is not a definition at all, so callers can
    /// fall through to ordinary evaluation.
    pub fn define_function(&mut self, input: &str) -> Result<Option<String>, CalcError> {
        let tokens = lex(input)?;
        let Some((name, params, start)) = userfn::definition_head(&tokens) else {
            return Ok(None);
        };
        let bad = |n: &str| {
            is_reserved(n) || self.registry.contains(n) || matches!(n, "rol" | "ror")
        };
        if bad(&name) {
            return Err(CalcError::Assignment(format!("'{name}' is a reserved name")));
        }
        if self.vars.contains_key(&name) {
            return Err(CalcError::Assignment(format!(
                "'{name}' is already a variable; remove it first"
            )));
        }
        for (i, p) in params.iter().enumerate() {
            if bad(p) || *p == name || self.user_fns.contains_key(p) {
                return Err(CalcError::Assignment(format!(
                    "'{p}' cannot be used as a parameter name"
                )));
            }
            if params[..i].contains(p) {
                return Err(CalcError::Assignment(format!("parameter '{p}' appears twice")));
            }
        }
        let body_tokens = &tokens[start..];
        if body_tokens.len() <= 1 {
            return Err(CalcError::Syntax("the function has no body".into()));
        }
        let (registry, fns) = (&self.registry, &self.user_fns);
        let body = parse(body_tokens, &|n| {
            n != name && (registry.contains(n) || fns.contains_key(n))
        })?;
        if matches!(body, Expr::Assign(..)) {
            return Err(CalcError::Syntax("a function body cannot be an assignment".into()));
        }
        if userfn::uses_var(&body, &name) {
            return Err(CalcError::Assignment(format!(
                "'{name}' cannot call itself (recursion is not supported)"
            )));
        }
        let mut called = std::collections::HashSet::new();
        userfn::calls(&body, &mut called);
        for c in &called {
            if userfn::reaches(&self.user_fns, c, &name) {
                return Err(CalcError::Assignment(format!(
                    "'{name}' and '{c}' would call each other (recursion is not supported)"
                )));
            }
        }
        // Reject bodies that are broken regardless of arguments (arity of
        // user functions, size limits).
        let mut trial = self.user_fns.clone();
        let body_text: String = input
            .chars()
            .skip(body_tokens[0].pos.saturating_sub(1))
            .collect::<String>()
            .trim()
            .to_string();
        let text = format!("{name}({}) = {body_text}", params.join(", "));
        trial.insert(
            name.clone(),
            UserFn {
                params,
                body,
                text: text.clone(),
            },
        );
        userfn::inline(&Expr::Call(name.clone(), trial[&name].params.iter().map(|p| Expr::Var(p.clone())).collect()), &trial)?;
        self.user_fns = trial;
        Ok(Some(text))
    }

    /// `(name, "name(params) = body")` for every user function, sorted.
    pub fn functions(&self) -> Vec<(String, String)> {
        let mut v: Vec<_> = self
            .user_fns
            .iter()
            .map(|(k, f)| (k.clone(), f.text.clone()))
            .collect();
        v.sort();
        v
    }

    /// Forgets one function; returns whether it existed.
    pub fn remove_function(&mut self, name: &str) -> bool {
        self.user_fns.remove(&name.trim().to_lowercase()).is_some()
    }

    pub fn clear_functions(&mut self) {
        self.user_fns.clear();
    }

    /// Compiles `input` into a fast `f64` function of `var` for plotting.
    /// `input` is an expression (`sin(x)/x`), a call of a user function
    /// (`f(x)`), or a definition (`f(t) = t^2`, which is compiled but not
    /// stored; its first parameter is the variable). Other variables are
    /// read from the session when this is called, so recompile after they change.
    pub fn plot_fn(&mut self, input: &str, var: &str) -> Result<PlotFn, CalcError> {
        let tokens = lex(input)?;
        let (registry, fns) = (&self.registry, &self.user_fns);
        let (expr, var) = match userfn::definition_head(&tokens) {
            Some((name, params, start)) => {
                let body_tokens = &tokens[start..];
                let body = parse(body_tokens, &|n| {
                    n != name && (registry.contains(n) || fns.contains_key(n))
                })?;
                let body = userfn::inline(&body, fns)?;
                let v = params.first().cloned().unwrap_or_else(|| var.to_string());
                (body, v)
            }
            None => (self.parse_expression(input)?, var.trim().to_lowercase()),
        };
        let (ans, mem) = (self.ans.clone(), self.memory.clone());
        let vars = &self.vars;
        let angle = self.ctx.angle;
        let ctx = &mut self.ctx;
        {
            let mut lookup = |name: &str| -> Result<f64, CalcError> {
                let n = match name {
                    "ans" => Some(&ans),
                    "mem" => mem.as_ref(),
                    _ => vars.get(name),
                };
                let n = n.ok_or_else(|| CalcError::UnknownIdentifier(name.to_string()))?;
                ctx.to_f64(n)
                    .ok_or_else(|| CalcError::Domain(format!("'{name}' is too large to plot")))
            };
            fasteval::compile(&expr, &var, &self.registry, angle, &mut lookup)
        }
    }

    pub fn is_function_name(&self, name: &str) -> bool {
        self.registry.contains(name) || self.user_fns.contains_key(name)
    }

    // --------------------------------------------------------------- calculus

    /// Parses `input` as a plain expression (no assignment).
    fn parse_expression(&self, input: &str) -> Result<Expr, CalcError> {
        let expr = self.parse_statement(input)?;
        if matches!(expr, Expr::Assign(..)) {
            return Err(CalcError::Syntax(
                "enter an expression, not an assignment".into(),
            ));
        }
        Ok(expr)
    }

    fn check_variable_name(&self, name: &str) -> Result<(), CalcError> {
        let tokens = lex(name)?;
        let single_name = tokens.len() == 2 && matches!(tokens[0].tok, crate::lexer::Tok::Ident(_));
        if !single_name
            || is_reserved(name)
            || self.registry.contains(name)
            || self.user_fns.contains_key(name)
        {
            return Err(CalcError::Syntax(format!(
                "'{}' cannot be used as the variable of differentiation",
                name.trim()
            )));
        }
        Ok(())
    }

    /// Symbolic derivative of `input` with respect to `var`, `order` times
    /// (1 to 10). Trigonometric functions follow the current angle mode; other
    /// names, including user variables, are treated as constants.
    pub fn differentiate(
        &self,
        input: &str,
        var: &str,
        order: usize,
    ) -> Result<Derivative, CalcError> {
        let var = var.trim().to_lowercase();
        self.check_variable_name(&var)?;
        if !(1..=10).contains(&order) {
            return Err(CalcError::Domain("the order must be between 1 and 10".into()));
        }
        let expr = self.parse_expression(input)?;
        symbolic::differentiate(&expr, &var, order, self.ctx.angle)
    }

    /// Evaluates a plain expression without touching the session: no history
    /// entry, no `ans`. Used for derived values such as a point of evaluation.
    pub fn eval_quiet(&mut self, input: &str) -> Result<Num, CalcError> {
        let expr = self.parse_expression(input)?;
        let mut env = Env {
            ctx: &mut self.ctx,
            registry: &self.registry,
            vars: &self.vars,
            memory: self.memory.as_ref(),
            ans: &self.ans,
        };
        eval(&expr, &mut env)
    }

    /// Evaluates `input` with `var` temporarily set to `value`. Unlike
    /// [`Calculator::eval`] this changes nothing in the session: no history
    /// entry, no `ans`, no stored variable.
    pub fn eval_with(&mut self, input: &str, var: &str, value: &Num) -> Result<Num, CalcError> {
        let var = var.trim().to_lowercase();
        self.check_variable_name(&var)?;
        let expr = self.parse_expression(input)?;
        let mut vars = self.vars.clone();
        vars.insert(var, value.clone());
        let mut env = Env {
            ctx: &mut self.ctx,
            registry: &self.registry,
            vars: &vars,
            memory: self.memory.as_ref(),
            ans: &self.ans,
        };
        eval(&expr, &mut env)
    }

    // -------------------------------------------------------------- extension

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Mutable access, used to register additional functions.
    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }
}
