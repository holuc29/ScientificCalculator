//! egui desktop frontend for the scicalc engine.
//!
//! The expression line is an ordinary text field: type into it (keyboard) or
//! build it with the buttons, then press Enter or `=`. After `=`, pressing an
//! operator button continues from the result (`ans`).

mod graph;
mod typeset;

use calc_core::programmer::parse_in_base;
use calc_core::{
    AngleMode, Base, Calculator, Derivative, Num, ProgValue, WordSize, MAX_DISPLAY_DIGITS,
};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Vec2};

const BUTTON_HEIGHT: f32 = 40.0;

/// What a button press asks the app to do.
#[derive(Clone, Debug)]
enum Action {
    /// Plain text (digits, parentheses, constants); starts fresh after `=`.
    Text(String),
    /// Binary or postfix operator; continues from `ans` after `=`.
    Operator(String),
    /// A function name; inserts `name(`.
    Function(String),
    Clear,
    Backspace,
    Equals,
    Negate,
    ToggleSecond,
    ToggleHyper,
    MemClear,
    MemRecall,
    MemAdd,
    MemSubtract,
    MemStore,
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Operator,
    Accent,
    Toggle(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Calculator,
    Derivative,
    Programmer,
    Graph,
}

/// State of the "Derivative" tab.
struct DerivState {
    /// The function, e.g. `x^2*sin(x)`.
    input: String,
    /// Variable of differentiation.
    var: String,
    order: usize,
    /// Where to evaluate (an expression, e.g. `pi/4`).
    point: String,
    result: Option<Derivative>,
    /// Angle mode the result was computed in (it affects trig derivatives).
    computed_in: AngleMode,
    error: Option<String>,
    /// `f(x0)` and `f'(x0)` as text, or an error message.
    at_point: Option<Result<(String, String), String>>,
}

impl DerivState {
    fn new() -> Self {
        DerivState {
            input: String::new(),
            var: "x".into(),
            order: 1,
            point: String::new(),
            result: None,
            computed_in: AngleMode::default(),
            error: None,
            at_point: None,
        }
    }
}

#[derive(Default)]
struct VarForm {
    name: String,
    value: String,
    error: Option<String>,
}


/// State of the "Programmer" tab.
struct ProgState {
    word: WordSize,
    input: String,
    value: ProgValue,
    /// Text of the HEX, DEC, OCT and BIN boxes (in `Base::ALL` order).
    fields: [String; 4],
    field_errors: [Option<String>; 4],
    error: Option<String>,
    /// Name of the variable the last statement assigned, if any.
    assigned: Option<String>,
    /// Move the cursor into the expression line (after a button inserted text).
    focus_input: bool,
}

impl ProgState {
    fn new() -> Self {
        let word = WordSize::default();
        let mut s = ProgState {
            word,
            input: String::new(),
            value: ProgValue::zero(word),
            fields: Default::default(),
            field_errors: Default::default(),
            error: None,
            assigned: None,
            focus_input: false,
        };
        s.refresh_fields(None);
        s
    }

    /// Rewrites the base boxes from the value (except box `keep`, which the
    /// user is typing in).
    fn refresh_fields(&mut self, keep: Option<usize>) {
        for (i, base) in Base::ALL.into_iter().enumerate() {
            self.field_errors[i] = None;
            if Some(i) != keep {
                self.fields[i] = self.value.format(base, true);
            }
        }
    }

    fn set_value(&mut self, v: ProgValue, keep: Option<usize>) {
        self.value = v;
        self.refresh_fields(keep);
    }

    fn set_word(&mut self, word: WordSize) {
        if word != self.word {
            self.word = word;
            let v = self.value.resized(word);
            self.set_value(v, None);
        }
    }

    /// The user edited base box `i`.
    fn edited(&mut self, i: usize) {
        match parse_in_base(&self.fields[i], Base::ALL[i], self.word) {
            Ok(v) => self.set_value(v, Some(i)),
            Err(e) => self.field_errors[i] = Some(e.to_string()),
        }
    }
}

struct CalcApp {
    prog: ProgState,
    graph: graph::GraphState,
    var_form: VarForm,
    typeset: typeset::Typesetter,
    tab: Tab,
    deriv: DerivState,
    calc: Calculator,
    expr: String,
    /// Value of the last successful evaluation (kept so changing the digit
    /// setting can re-render it).
    last: Option<Num>,
    result_text: String,
    error: Option<String>,
    second: bool,
    hyper: bool,
    digits: usize,
    /// True right after `=`: operator buttons then continue from the result.
    just_evaluated: bool,
    cursor_to_end: bool,
}

impl CalcApp {
    fn new() -> Self {
        let calc = Calculator::new();
        let digits = calc.display_digits();
        CalcApp {
            prog: ProgState::new(),
            graph: graph::GraphState::new(),
            var_form: VarForm::default(),
            typeset: typeset::Typesetter::default(),
            tab: Tab::Calculator,
            deriv: DerivState::new(),
            calc,
            expr: String::new(),
            last: None,
            result_text: String::new(),
            error: None,
            second: false,
            hyper: false,
            digits,
            just_evaluated: false,
            cursor_to_end: true,
        }
    }

    // ------------------------------------------------------------- editing

    fn start_fresh_if_needed(&mut self) {
        if self.just_evaluated {
            self.expr.clear();
            self.just_evaluated = false;
        }
    }

    fn append_text(&mut self, text: &str) {
        self.start_fresh_if_needed();
        self.expr.push_str(text);
    }

    fn append_operator(&mut self, text: &str) {
        if self.just_evaluated || self.expr.trim().is_empty() {
            // Continue from the previous result (a lone leading '-' is a sign).
            if !(self.expr.trim().is_empty() && text.trim() == "-") {
                self.expr = "ans".to_string();
            }
            self.just_evaluated = false;
        }
        self.expr.push_str(text);
    }

    fn backspace(&mut self) {
        self.just_evaluated = false;
        if self.expr.ends_with('(') {
            // Remove a whole "name(" token together.
            self.expr.pop();
            while self
                .expr
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                self.expr.pop();
            }
        } else {
            self.expr.pop();
        }
    }

    /// Flips the sign of the trailing number (or of the whole expression).
    fn negate(&mut self) {
        self.just_evaluated = false;
        if self.expr.is_empty() {
            self.expr.push('-');
            return;
        }
        if let Some(inner) = self
            .expr
            .strip_prefix("-(")
            .and_then(|s| s.strip_suffix(')'))
        {
            self.expr = inner.to_string();
            return;
        }
        let bytes = self.expr.as_bytes();
        let end = bytes.len();
        let mut start = end;
        while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == b'.') {
            start -= 1;
        }
        if start < end {
            let before = self.expr[..start].chars().last();
            if before == Some('-') {
                let prefix = &self.expr[..start - 1];
                let unary = prefix
                    .chars()
                    .last()
                    .is_none_or(|c| "(+-*/^,×÷ ".contains(c));
                if unary {
                    self.expr.remove(start - 1);
                    return;
                }
            }
            self.expr.insert(start, '-');
        } else {
            self.expr = format!("-({})", self.expr);
        }
    }

    // ---------------------------------------------------------- evaluation

    fn equals(&mut self) {
        if self.expr.trim().is_empty() {
            return;
        }
        // Close any parentheses the user left open.
        let open = self.expr.matches('(').count();
        let close = self.expr.matches(')').count();
        for _ in close..open {
            self.expr.push(')');
        }
        match self.calc.define_function(&self.expr) {
            Ok(Some(text)) => {
                self.result_text = format!("defined {text}");
                self.error = None;
                self.just_evaluated = false;
                return;
            }
            Ok(None) => {}
            Err(e) => {
                self.error = Some(e.to_string());
                self.just_evaluated = false;
                return;
            }
        }
        match self.calc.eval(&self.expr) {
            Ok(v) => {
                self.result_text = self.calc.format(&v);
                self.last = Some(v);
                self.error = None;
                self.just_evaluated = true;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.just_evaluated = false;
            }
        }
    }

    /// The value memory operations should use: the current expression
    /// evaluated, or the last result.
    fn current_value(&mut self) -> Option<Num> {
        if !self.expr.trim().is_empty() && !self.just_evaluated {
            self.equals();
        }
        if self.error.is_none() {
            self.last.clone()
        } else {
            None
        }
    }

    fn set_digits(&mut self, digits: usize) {
        self.digits = digits;
        self.calc.set_display_digits(digits);
        if let Some(v) = self.last.clone() {
            self.result_text = self.calc.format(&v);
        }
    }

    fn apply(&mut self, action: Action) {
        self.cursor_to_end = true;
        if !matches!(action, Action::ToggleSecond | Action::ToggleHyper) {
            // Any edit dismisses the previous error message.
            self.error = None;
        }
        match action {
            Action::Text(t) => self.append_text(&t),
            Action::Operator(t) => self.append_operator(&t),
            Action::Function(name) => self.append_text(&format!("{name}(")),
            Action::Clear => {
                self.expr.clear();
                self.last = None;
                self.result_text.clear();
                self.error = None;
                self.just_evaluated = false;
            }
            Action::Backspace => self.backspace(),
            Action::Equals => self.equals(),
            Action::Negate => self.negate(),
            Action::ToggleSecond => self.second = !self.second,
            Action::ToggleHyper => self.hyper = !self.hyper,
            Action::MemClear => self.calc.memory_clear(),
            Action::MemRecall => {
                if self.calc.memory().is_some() {
                    self.append_text("mem");
                }
            }
            Action::MemStore => {
                if let Some(v) = self.current_value() {
                    self.calc.memory_store(v);
                }
            }
            Action::MemAdd => {
                if let Some(v) = self.current_value() {
                    if let Err(e) = self.calc.memory_add(&v) {
                        self.error = Some(e.to_string());
                    }
                }
            }
            Action::MemSubtract => {
                if let Some(v) = self.current_value() {
                    if let Err(e) = self.calc.memory_subtract(&v) {
                        self.error = Some(e.to_string());
                    }
                }
            }
        }
    }


    // ------------------------------------------------------------ programmer

    /// Appends text to the Programmer expression (used by the operator buttons).
    fn prog_insert(&mut self, text: &str) {
        self.prog.input.push_str(text);
        self.prog.error = None;
        self.prog.focus_input = true;
    }

    fn run_programmer(&mut self) {
        self.prog.assigned = None;
        if self.prog.input.trim().is_empty() {
            self.prog.error = None;
            return;
        }
        match self.calc.programmer_eval(&self.prog.input, self.prog.word) {
            Ok(out) => {
                self.prog.error = None;
                self.prog.assigned = out.assigned;
                self.prog.set_value(out.value, None);
                if self.deriv.result.is_some() {
                    self.refresh_point();
                }
            }
            Err(e) => self.prog.error = Some(e.to_string()),
        }
    }

    // ---------------------------------------------------------- derivatives

    /// Differentiates the entered function and refreshes the point values.
    fn run_derivative(&mut self) {
        let d = &mut self.deriv;
        d.at_point = None;
        if d.input.trim().is_empty() {
            d.result = None;
            d.error = None;
            return;
        }
        match self.calc.differentiate(&d.input, &d.var, d.order) {
            Ok(res) => {
                d.result = Some(res);
                d.computed_in = self.calc.angle_mode();
                d.error = None;
            }
            Err(e) => {
                d.result = None;
                d.error = Some(e.to_string());
            }
        }
        self.refresh_point();
    }

    /// Evaluates the function and its derivative at the chosen point.
    fn refresh_point(&mut self) {
        self.deriv.at_point = None;
        let Some(res) = self.deriv.result.clone() else { return };
        if self.deriv.point.trim().is_empty() {
            return;
        }
        let out = point_values(&mut self.calc, &self.deriv, &res.expr);
        self.deriv.at_point = Some(out);
    }

    /// Moves the derivative to the calculator tab as an expression to edit.
    fn derivative_to_calculator(&mut self) {
        if let Some(r) = &self.deriv.result {
            self.expr = r.expr.clone();
            self.just_evaluated = false;
            self.error = None;
            self.cursor_to_end = true;
            self.tab = Tab::Calculator;
        }
    }

    // ------------------------------------------------------------------ UI

    fn display_ui(&mut self, ui: &mut egui::Ui) {
        let id = egui::Id::new("expression");

        if self.cursor_to_end {
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
                let end = egui::text::CCursor::new(self.expr.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), id);
            }
            ui.memory_mut(|m| m.request_focus(id));
            self.cursor_to_end = false;
        }

        let response = ui.add(
            egui::TextEdit::singleline(&mut self.expr)
                .id(id)
                .font(FontId::monospace(20.0))
                .desired_width(f32::INFINITY)
                .hint_text("type an expression, e.g. sin(30) + 2^10"),
        );
        if response.changed() {
            self.just_evaluated = false;
            self.error = None;
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if response.lost_focus() && enter {
            self.equals();
            self.cursor_to_end = true;
        }
        if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.apply(Action::Clear);
        }

        ui.add_space(4.0);
        // A fixed height is essential: a bare right-to-left layout would claim
        // all remaining vertical space and push the keypad off-screen.
        let row = Vec2::new(ui.available_width(), 42.0);
        ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
            let avail = ui.available_width();
            if let Some(err) = &self.error {
                let size = fit_size(avail, err.chars().count(), 16.0, 11.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(err)
                            .size(size)
                            .color(Color32::from_rgb(220, 80, 80)),
                    )
                    .truncate(),
                )
                .on_hover_text(err);
            } else {
                let text = if self.result_text.is_empty() {
                    "0"
                } else {
                    &self.result_text
                };
                // Long results shrink to fit and never widen the panel.
                let size = fit_size(avail, text.chars().count(), 30.0, 13.0);
                ui.add(
                    egui::Label::new(RichText::new(text).size(size).monospace().strong())
                        .selectable(true)
                        .truncate(),
                )
                .on_hover_text(text);
            }
        });
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut mode = self.calc.angle_mode();
            for (m, label) in [
                (AngleMode::Degrees, "DEG"),
                (AngleMode::Radians, "RAD"),
                (AngleMode::Gradians, "GRAD"),
            ] {
                ui.selectable_value(&mut mode, m, label);
            }
            if mode != self.calc.angle_mode() {
                self.calc.set_angle_mode(mode);
            }
            ui.separator();
            ui.label("Digits");
            let mut digits = self.digits;
            ui.add(egui::DragValue::new(&mut digits).range(1..=MAX_DISPLAY_DIGITS));
            if digits != self.digits {
                self.set_digits(digits);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(m) = self.calc.memory().cloned() {
                    let text = self.calc.format(&m);
                    ui.add(
                        egui::Label::new(RichText::new(format!("M = {text}")).weak()).truncate(),
                    )
                    .on_hover_text(text);
                }
            });
        });
    }


    fn programmer_ui(&mut self, ui: &mut egui::Ui) {
        let mut run = false;
        let input_id = egui::Id::new("prog_input");
        if self.prog.focus_input {
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), input_id) {
                let end = egui::text::CCursor::new(self.prog.input.chars().count());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), input_id);
            }
            ui.memory_mut(|m| m.request_focus(input_id));
            self.prog.focus_input = false;
        }

        ui.label(RichText::new("Expression").weak());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.prog.input)
                .id(input_id)
                .font(FontId::monospace(20.0))
                .desired_width(f32::INFINITY)
                .hint_text("e.g. (a << 4) | 0xF"),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            run = true;
        }
        if resp.changed() {
            self.prog.error = None;
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if ui.button("Evaluate").clicked() {
                run = true;
            }
            ui.separator();
            ui.label("Bits");
            let mut word = self.prog.word;
            for w in WordSize::ALL {
                ui.selectable_value(&mut word, w, w.bits().to_string());
            }
            self.prog.set_word(word);
        });
        if run {
            self.run_programmer();
        }
        if let Some(err) = &self.prog.error {
            ui.add(egui::Label::new(RichText::new(err).color(Color32::from_rgb(230, 80, 80))).wrap());
        } else if let Some(name) = &self.prog.assigned {
            ui.label(RichText::new(format!("Stored in variable '{name}'.")).weak());
        }

        ui.add_space(6.0);
        if let Some(text) = operator_buttons(ui) {
            self.prog_insert(text);
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // The four bases: edit any box and the others follow.
        let mut edited: Option<usize> = None;
        for (i, base) in Base::ALL.into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [38.0, 22.0],
                    egui::Label::new(RichText::new(base.name()).monospace().weak()),
                );
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.prog.fields[i])
                        .font(FontId::monospace(15.0))
                        .desired_width(f32::INFINITY),
                );
                if r.changed() {
                    edited = Some(i);
                }
            });
            if let Some(e) = &self.prog.field_errors[i] {
                ui.add(egui::Label::new(RichText::new(e).small().color(Color32::from_rgb(230, 80, 80))).wrap());
            }
        }
        if let Some(i) = edited {
            self.prog.edited(i);
        }
        let v = self.prog.value;
        if v.signed() < 0 {
            ui.label(RichText::new(format!("unsigned: {}", v.unsigned())).small().weak());
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Bits (click to flip)").weak());
        if let Some(bit) = bit_grid(ui, v) {
            let flipped = v.toggled(bit);
            self.prog.set_value(flipped, None);
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Whole numbers only; variables must be whole numbers that fit the bit size. \
                 Operators: + - * / % mod  & | ^ ~  << >> >>>  nand nor xor.  \
                 Functions: rol(x, n), ror(x, n).  Literals: 255, 0xFF, 0b1010, 0o17.  \
                 Arithmetic wraps around like hardware.",
            )
            .small()
            .weak(),
        );
    }

    fn derivative_ui(&mut self, ui: &mut egui::Ui) {
        // Trigonometric derivatives depend on the angle mode: keep them in sync.
        if self.deriv.result.is_some() && self.deriv.computed_in != self.calc.angle_mode() {
            self.run_derivative();
        }
        let mut run = false;

        ui.label(RichText::new("Function").weak());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.deriv.input)
                .font(FontId::monospace(20.0))
                .desired_width(f32::INFINITY)
                .hint_text("e.g. x^2*sin(x)"),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            run = true;
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("With respect to");
            let var_resp = ui.add(
                egui::TextEdit::singleline(&mut self.deriv.var)
                    .font(FontId::monospace(16.0))
                    .char_limit(16)
                    .desired_width(56.0),
            );
            if var_resp.lost_focus() {
                run = true;
            }
            ui.separator();
            ui.label("Order");
            let before = self.deriv.order;
            ui.add(egui::DragValue::new(&mut self.deriv.order).range(1..=10));
            if self.deriv.order != before {
                run = true;
            }
            if ui.button("Differentiate").clicked() {
                run = true;
            }
        });
        if run {
            self.run_derivative();
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let var = self.deriv.var.trim().to_string();
        if let Some(err) = &self.deriv.error {
            ui.add(egui::Label::new(RichText::new(err).color(Color32::from_rgb(230, 80, 80))).wrap());
        } else if let Some(res) = self.deriv.result.clone() {
            let marks = match self.deriv.order {
                1 => "'".to_string(),
                2 => "''".to_string(),
                n => format!("^({n})"),
            };
            ui.label(RichText::new(format!("f{marks}({var}) =")).weak());
            egui::ScrollArea::horizontal()
                .id_salt("typeset_result")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if !self.typeset.show(ui, &res.latex, 26.0) {
                        ui.add(
                            egui::Label::new(RichText::new(&res.expr).monospace().size(18.0))
                                .selectable(true)
                                .wrap(),
                        );
                    }
                });
            ui.add(
                egui::Label::new(RichText::new(&res.expr).monospace().size(12.0).weak())
                    .selectable(true)
                    .truncate(),
            )
            .on_hover_text("Plain-text form (selectable; this is what Copy to calculator uses)");
            for note in &res.notes {
                ui.add_space(2.0);
                ui.add(egui::Label::new(RichText::new(note).small().weak()).wrap());
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Copy to calculator").clicked() {
                    self.derivative_to_calculator();
                }
                if ui
                    .button("Use as function")
                    .on_hover_text("Put the result into the function field (differentiate it again)")
                    .clicked()
                {
                    self.deriv.input = res.expr.clone();
                    self.deriv.order = 1;
                    self.run_derivative();
                }
            });
        } else {
            ui.label(
                RichText::new(
                    "Enter a function and press Enter. Names other than the variable \
                     (including those in the Variables panel) are treated as constants. \
                     Trigonometric functions follow the DEG/RAD/GRAD setting.",
                )
                .weak(),
            );
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Evaluate at");
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.deriv.point)
                    .font(FontId::monospace(16.0))
                    .desired_width(160.0)
                    .hint_text("e.g. pi/4 or 2.5"),
            );
            if r.changed() {
                self.refresh_point();
            }
        });
        match self.deriv.at_point.clone() {
            Some(Ok((f, df))) => {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(RichText::new(format!("f({var}0)  = {f}")).monospace())
                        .selectable(true)
                        .truncate(),
                );
                ui.add(
                    egui::Label::new(RichText::new(format!("f'({var}0) = {df}")).monospace())
                        .selectable(true)
                        .truncate(),
                );
            }
            Some(Err(e)) => {
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(e).color(Color32::from_rgb(230, 80, 80))).wrap());
            }
            None => {}
        }
    }

    fn keypad_ui(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let second = self.second;
        let hyper = self.hyper;

        let text = |s: &str| Action::Text(s.to_string());
        let op = |s: &str| Action::Operator(s.to_string());
        let func = |s: &str| Action::Function(s.to_string());

        // Memory row.
        key_row(ui, "memory", &mut action, vec![
            ("MC", Action::MemClear, Style::Plain),
            ("MR", Action::MemRecall, Style::Plain),
            ("M+", Action::MemAdd, Style::Plain),
            ("M−", Action::MemSubtract, Style::Plain),
            ("MS", Action::MemStore, Style::Plain),
        ]);

        // Trigonometry row: 2nd selects inverse, hyp selects hyperbolic.
        let trig = |base: &str| -> (String, Action) {
            let name = trig_name(base, second, hyper);
            (name.clone(), Action::Function(name))
        };
        let mut trig_a = vec![("hyp".to_string(), Action::ToggleHyper, Style::Toggle(hyper))];
        for base in ["sin", "cos", "tan", "sec"] {
            let (label, act) = trig(base);
            trig_a.push((label, act, Style::Plain));
        }
        key_row_owned(ui, "trig_a", &mut action, trig_a);
        let mut trig_b = Vec::new();
        for base in ["csc", "cot"] {
            let (label, act) = trig(base);
            trig_b.push((label, act, Style::Plain));
        }
        key_row_owned(ui, "trig_b", &mut action, trig_b);

        // Extra functions.
        key_row(ui, "extra", &mut action, vec![
            ("nCr", func("ncr"), Style::Plain),
            ("nPr", func("npr"), Style::Plain),
            ("dms", func("dms"), Style::Plain),
            ("deg", func("deg"), Style::Plain),
            ("rand", text("rand()"), Style::Plain),
        ]);

        // Main keypad (same layout as Windows' scientific mode).
        let pick = |a: (&'static str, Action), b: (&'static str, Action)| if second { b } else { a };
        let (l_sq, a_sq) = pick(("x²", op("²")), ("x³", op("³")));
        let (l_rt, a_rt) = pick(("√x", func("sqrt")), ("³√x", func("cbrt")));
        let (l_pw, a_pw) = pick(("x^y", op("^")), ("y√x", func("root")));
        let (l_10, a_10) = pick(("10^x", text("10^")), ("2^x", text("2^")));
        let (l_lg, a_lg) = pick(("log", func("log")), ("logb", func("logb")));
        let (l_ln, a_ln) = pick(("ln", func("ln")), ("e^x", text("e^")));

        key_row(ui, "r0", &mut action, vec![
            ("2nd", Action::ToggleSecond, Style::Toggle(second)),
            ("π", text("pi"), Style::Plain),
            ("e", text("e"), Style::Plain),
            ("C", Action::Clear, Style::Operator),
            ("Bksp", Action::Backspace, Style::Operator),
        ]);
        key_row(ui, "r1", &mut action, vec![
            (l_sq, a_sq, Style::Plain),
            ("1/x", op("^(-1)"), Style::Plain),
            ("|x|", func("abs"), Style::Plain),
            ("mod", op(" mod "), Style::Plain),
            ("n!", op("!"), Style::Plain),
        ]);
        key_row(ui, "r2", &mut action, vec![
            (l_rt, a_rt, Style::Plain),
            ("(", text("("), Style::Plain),
            (")", text(")"), Style::Plain),
            (l_pw, a_pw, Style::Plain),
            ("÷", op("÷"), Style::Operator),
        ]);
        key_row(ui, "r3", &mut action, vec![
            (l_10, a_10, Style::Plain),
            ("7", text("7"), Style::Plain),
            ("8", text("8"), Style::Plain),
            ("9", text("9"), Style::Plain),
            ("×", op("×"), Style::Operator),
        ]);
        key_row(ui, "r4", &mut action, vec![
            (l_lg, a_lg, Style::Plain),
            ("4", text("4"), Style::Plain),
            ("5", text("5"), Style::Plain),
            ("6", text("6"), Style::Plain),
            ("−", op("-"), Style::Operator),
        ]);
        key_row(ui, "r5", &mut action, vec![
            (l_ln, a_ln, Style::Plain),
            ("1", text("1"), Style::Plain),
            ("2", text("2"), Style::Plain),
            ("3", text("3"), Style::Plain),
            ("+", op("+"), Style::Operator),
        ]);
        key_row(ui, "r6", &mut action, vec![
            (",", text(", "), Style::Plain),
            ("±", Action::Negate, Style::Plain),
            ("0", text("0"), Style::Plain),
            (".", text("."), Style::Plain),
            ("=", Action::Equals, Style::Accent),
        ]);

        action
    }

    /// Lists user-defined variables; clicking one inserts its name.
    fn variables_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Variables");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.calc.clear_variables();
                }
            });
        });
        ui.separator();

        // Entry form: [name] = [value] [Set]. Tab moves between the fields.
        let name_id = egui::Id::new("var_name");
        let value_id = egui::Id::new("var_value");
        let mut submit = false;
        ui.horizontal(|ui| {
            let r1 = ui.add(
                egui::TextEdit::singleline(&mut self.var_form.name)
                    .id(name_id)
                    .font(FontId::monospace(14.0))
                    .desired_width(54.0)
                    .char_limit(24)
                    .hint_text("name"),
            );
            ui.label("=");
            let room = (ui.available_width() - 48.0).max(60.0);
            let r2 = ui.add(
                egui::TextEdit::singleline(&mut self.var_form.value)
                    .id(value_id)
                    .font(FontId::monospace(14.0))
                    .desired_width(room)
                    .hint_text("value, e.g. 2*pi"),
            );
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (r1.lost_focus() || r2.lost_focus()) && enter {
                submit = true;
            }
            if ui.button("Set").clicked() {
                submit = true;
            }
            if r1.changed() || r2.changed() {
                self.var_form.error = None;
            }
        });
        if submit {
            match self.calc.set_variable(&self.var_form.name, &self.var_form.value) {
                Ok(_) => {
                    self.var_form = VarForm::default();
                    ui.memory_mut(|m| m.request_focus(name_id));
                    if self.deriv.result.is_some() {
                        self.refresh_point();
                    }
                }
                Err(e) => self.var_form.error = Some(e.to_string()),
            }
        }
        if let Some(err) = &self.var_form.error {
            ui.add(egui::Label::new(RichText::new(err).small().color(Color32::from_rgb(230, 80, 80))).wrap());
        }
        ui.add_space(4.0);

        let vars: Vec<(String, Num)> = self
            .calc
            .variables()
            .into_iter()
            .map(|(n, v)| (n.to_string(), v.clone()))
            .collect();
        let mut insert: Option<String> = None;
        let mut remove: Option<String> = None;
        let mut edit: Option<(String, String)> = None;
        egui::ScrollArea::vertical()
            .id_salt("variables")
            .max_height(150.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if vars.is_empty() {
                    ui.label(RichText::new("None yet.").weak());
                }
                for (name, value) in &vars {
                    let text = self.calc.format(value);
                    let width = ui.available_width();
                    ui.horizontal(|ui| {
                        ui.allocate_ui(Vec2::new((width - 70.0).max(40.0), 18.0), |ui| {
                            let resp = ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(format!("{name} = {text}")).monospace(),
                                    )
                                    .truncate()
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_text(format!("{name} = {text}\nClick to insert the name"));
                            if resp.clicked() {
                                insert = Some(name.clone());
                            }
                        });
                        if ui
                            .small_button("edit")
                            .on_hover_text("Load into the form above")
                            .clicked()
                        {
                            edit = Some((name.clone(), text.clone()));
                        }
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            remove = Some(name.clone());
                        }
                    });
                }
            });
        if let Some(name) = insert {
            match self.tab {
                Tab::Calculator => self.apply(Action::Text(name)),
                Tab::Derivative => self.deriv.input.push_str(&name),
                Tab::Programmer => self.prog.input.push_str(&name),
                Tab::Graph => self.graph.add_curve(&name),
            }
        }
        if let Some(name) = remove {
            self.calc.remove_variable(&name);
        }
        if let Some((name, value)) = edit {
            self.var_form = VarForm {
                name,
                value,
                error: None,
            };
            ui.memory_mut(|m| m.request_focus(value_id));
        }
        ui.add_space(8.0);
    }

    /// Lists user-defined functions; clicking one inserts a call.
    fn functions_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Functions");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.calc.clear_functions();
                }
            });
        });
        ui.separator();
        let funcs = self.calc.functions();
        let mut insert: Option<String> = None;
        let mut remove: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("functions")
            .max_height(90.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if funcs.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new("None yet. Type e.g. f(x) = x^2 + 1 in the main field or a graph row.")
                                .weak(),
                        )
                        .wrap(),
                    );
                }
                for (name, text) in &funcs {
                    let width = ui.available_width();
                    ui.horizontal(|ui| {
                        ui.allocate_ui(Vec2::new((width - 30.0).max(40.0), 18.0), |ui| {
                            let resp = ui
                                .add(
                                    egui::Label::new(RichText::new(text).monospace())
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text(format!("{text}\nClick to use it"));
                            if resp.clicked() {
                                insert = Some(text.clone());
                            }
                        });
                        if ui.small_button("x").on_hover_text("Remove").clicked() {
                            remove = Some(name.clone());
                        }
                    });
                }
            });
        if let Some(text) = insert {
            let name = text.split('(').next().unwrap_or("").to_string();
            let head = text.split('=').next().unwrap_or("").trim().to_string();
            match self.tab {
                Tab::Calculator => self.apply(Action::Function(name)),
                Tab::Derivative => {
                    self.deriv.input.push_str(&name);
                    self.deriv.input.push('(');
                }
                Tab::Programmer => {}
                Tab::Graph => self.graph.add_curve(&head),
            }
        }
        if let Some(name) = remove {
            self.calc.remove_function(&name);
        }
        ui.add_space(8.0);
    }

    fn history_ui(&mut self, ui: &mut egui::Ui) {
        self.variables_ui(ui);
        self.functions_ui(ui);
        ui.horizontal(|ui| {
            ui.heading("History");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.calc.clear_history();
                }
            });
        });
        ui.separator();

        let mut recall: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.calc.history().is_empty() {
                    ui.label(RichText::new("Nothing yet.").weak());
                }
                for entry in self.calc.history().iter().rev() {
                    let resp = ui
                        .vertical(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.input).size(12.5).weak().monospace(),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("= {}", entry.output))
                                        .size(12.5)
                                        .monospace()
                                        .strong(),
                                )
                                .truncate(),
                            );
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_text("Click to reuse this expression");
                    if resp.clicked() {
                        recall = Some(entry.input.clone());
                    }
                    ui.separator();
                }
            });
        if let Some(input) = recall {
            self.expr = input;
            self.just_evaluated = false;
            self.cursor_to_end = true;
        }
    }
}



/// The bit-operation buttons: (caption, label, text to insert, tooltip).
const OPERATOR_KEYS: [(&str, &str, &str, &str); 14] = [
    ("AND", "&", " & ", "bitwise AND"),
    ("OR", "|", " | ", "bitwise OR"),
    ("XOR", "^", " ^ ", "bitwise exclusive OR"),
    ("NOT", "~", "~", "bitwise NOT (flips every bit)"),
    ("NAND", "nand", " nand ", "NOT (a AND b)"),
    ("NOR", "nor", " nor ", "NOT (a OR b)"),
    ("MOD", "%", " % ", "remainder (sign of the dividend)"),
    ("LSHIFT", "<<", " << ", "shift left"),
    ("RSHIFT", ">>", " >> ", "arithmetic shift right (keeps the sign)"),
    ("URSHIFT", ">>>", " >>> ", "logical shift right (fills with zeros)"),
    ("ROL", "rol", "rol(", "rotate left: rol(x, n)"),
    ("ROR", "ror", "ror(", "rotate right: ror(x, n)"),
    ("(", "(", "(", "open parenthesis"),
    (")", ")", ")", "close parenthesis"),
];

/// Draws the operator buttons (a small caption above each) in a fixed grid and
/// returns the text of the one that was clicked.
fn operator_buttons(ui: &mut egui::Ui) -> Option<&'static str> {
    const COLUMNS: usize = 7;
    let spacing = ui.spacing().item_spacing.x;
    let width = ((ui.available_width() - spacing * (COLUMNS as f32 - 1.0)) / COLUMNS as f32)
        .floor()
        .max(30.0);
    let mut clicked = None;
    for row in OPERATOR_KEYS.chunks(COLUMNS) {
        ui.horizontal(|ui| {
            for (caption, label, insert, tip) in row {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.add_sized(
                        [width, 12.0],
                        egui::Label::new(RichText::new(*caption).size(10.0).weak()),
                    );
                    let b = egui::Button::new(RichText::new(*label).monospace().size(15.0));
                    if ui.add_sized([width, 28.0], b).on_hover_text(*tip).clicked() {
                        clicked = Some(*insert);
                    }
                });
            }
        });
    }
    clicked
}

/// Draws the bits of `v` (most significant first, 16 per row, grouped in
/// fours). Returns the index of a bit that was clicked.
fn bit_grid(ui: &mut egui::Ui, v: ProgValue) -> Option<u32> {
    let total = v.word().bits();
    let per_row = total.min(16);
    let rows = total / per_row;
    let spacing = 2.0;
    let gap = 7.0;
    let groups = (per_row / 4).max(1) as f32;
    let label_w = 26.0;
    let avail = ui.available_width() - label_w - 6.0;
    let cell = ((avail - gap * (groups - 1.0) - spacing * (per_row as f32 - 1.0)) / per_row as f32)
        .clamp(12.0, 30.0);
    let mut clicked = None;
    for row in 0..rows {
        let top = total - 1 - row * per_row;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            ui.add_sized(
                [label_w, cell * 1.3],
                egui::Label::new(RichText::new(top.to_string()).small().weak()),
            );
            for k in 0..per_row {
                let index = top - k;
                if k > 0 && k % 4 == 0 {
                    ui.add_space(gap);
                }
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(cell, cell * 1.3), egui::Sense::CLICK);
                let on = v.bit(index);
                let mut fill = if on {
                    Color32::from_rgb(46, 110, 190)
                } else {
                    ui.visuals().widgets.inactive.weak_bg_fill
                };
                if resp.hovered() {
                    fill = fill.gamma_multiply(1.3);
                }
                ui.painter().rect_filled(rect, 3.0, fill);
                let color = if on { Color32::WHITE } else { ui.visuals().weak_text_color() };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if on { "1" } else { "0" },
                    FontId::monospace((cell * 0.6).clamp(9.0, 15.0)),
                    color,
                );
                if resp.on_hover_text(format!("bit {index}")).clicked() {
                    clicked = Some(index);
                }
            }
        });
    }
    clicked
}

/// `f(x0)` and `f'(x0)` as text for the derivative tab.
fn point_values(calc: &mut Calculator, d: &DerivState, derivative: &str) -> Result<(String, String), String> {
    let x0 = calc.eval_quiet(&d.point).map_err(|e| format!("point: {e}"))?;
    let f = calc
        .eval_with(&d.input, &d.var, &x0)
        .map_err(|e| format!("f at the point: {e}"))?;
    let df = calc
        .eval_with(derivative, &d.var, &x0)
        .map_err(|e| format!("derivative at the point: {e}"))?;
    Ok((calc.format(&f), calc.format(&df)))
}

/// Largest font size (between `min` and `max`) at which `chars` monospace
/// characters fit into `avail` pixels.
fn fit_size(avail: f32, chars: usize, max: f32, min: f32) -> f32 {
    let needed_per_pt = chars.max(1) as f32 * 0.62;
    (avail / needed_per_pt).clamp(min, max)
}

/// `sin` -> `asin` (2nd) -> `sinh` (hyp) -> `asinh` (both).
fn trig_name(base: &str, second: bool, hyper: bool) -> String {
    format!(
        "{}{}{}",
        if second { "a" } else { "" },
        base,
        if hyper { "h" } else { "" }
    )
}

/// Number of columns in the keypad grid. Every row uses the same cell width, so
/// buttons line up vertically; a row with fewer keys leaves its trailing cells
/// empty (each empty cell is exactly one grid cell wide).
const COLUMNS: usize = 5;

/// Draws one row of grid-aligned buttons; records a click in `action`.
fn key_row(
    ui: &mut egui::Ui,
    id: &str,
    action: &mut Option<Action>,
    keys: Vec<(&str, Action, Style)>,
) {
    key_row_owned(
        ui,
        id,
        action,
        keys.into_iter()
            .map(|(l, a, s)| (l.to_string(), a, s))
            .collect(),
    );
}

fn key_row_owned(
    ui: &mut egui::Ui,
    id: &str,
    action: &mut Option<Action>,
    keys: Vec<(String, Action, Style)>,
) {
    let spacing = ui.spacing().item_spacing.x;
    let n = COLUMNS as f32;
    let width = ((ui.available_width() - spacing * (n - 1.0)) / n).floor().max(24.0);
    let blanks = COLUMNS.saturating_sub(keys.len());
    ui.horizontal(|ui| {
        ui.push_id(id, |ui| {
            for (label, act, style) in keys {
                let mut text = RichText::new(label).size(16.0);
                let mut button;
                match style {
                    Style::Plain => button = egui::Button::new(text),
                    Style::Operator => {
                        text = text.strong();
                        button = egui::Button::new(text).fill(ui.visuals().widgets.inactive.weak_bg_fill.gamma_multiply(1.6));
                    }
                    Style::Accent => {
                        text = text.strong().color(Color32::WHITE);
                        button = egui::Button::new(text).fill(Color32::from_rgb(46, 110, 190));
                    }
                    Style::Toggle(on) => {
                        button = egui::Button::new(text).selected(on);
                    }
                }
                button = button.min_size(Vec2::new(width, BUTTON_HEIGHT));
                if ui.add_sized([width, BUTTON_HEIGHT], button).clicked() {
                    *action = Some(act);
                }
            }
            for _ in 0..blanks {
                ui.allocate_space(Vec2::new(width, BUTTON_HEIGHT));
            }
        });
    });
}

impl eframe::App for CalcApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::right("history")
            .exact_size(280.0)
            .resizable(false)
            .show(ui, |ui| self.history_ui(ui));

        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Calculator, "Calculator");
                ui.selectable_value(&mut self.tab, Tab::Derivative, "Derivative");
                ui.selectable_value(&mut self.tab, Tab::Programmer, "Programmer");
                ui.selectable_value(&mut self.tab, Tab::Graph, "Graph");
            });
            ui.separator();
            if self.tab != Tab::Programmer {
                self.settings_ui(ui);
                ui.add_space(4.0);
            }
            match self.tab {
                Tab::Calculator => {
                    self.display_ui(ui);
                    ui.add_space(6.0);
                    if let Some(action) = self.keypad_ui(ui) {
                        self.apply(action);
                    }
                }
                Tab::Derivative => self.derivative_ui(ui),
                Tab::Programmer => self.programmer_ui(ui),
                Tab::Graph => {
                    self.graph.curves_ui(ui, &mut self.calc);
                    ui.add_space(4.0);
                    self.graph.canvas_ui(ui, &mut self.calc);
                }
            }
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Scientific Calculator")
            .with_inner_size([760.0, 700.0])
            .with_min_inner_size([660.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "scicalc",
        options,
        Box::new(|_cc| Ok(Box::new(CalcApp::new()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> CalcApp {
        CalcApp::new()
    }

    fn text(s: &str) -> Action {
        Action::Text(s.into())
    }
    fn op(s: &str) -> Action {
        Action::Operator(s.into())
    }
    fn func(s: &str) -> Action {
        Action::Function(s.into())
    }

    fn press(app: &mut CalcApp, actions: Vec<Action>) {
        for a in actions {
            app.apply(a);
        }
    }

    #[test]
    fn simple_calculation() {
        let mut a = app();
        press(&mut a, vec![text("2"), op("+"), text("3"), Action::Equals]);
        assert_eq!(a.result_text, "5");
        assert_eq!(a.error, None);
    }

    #[test]
    fn operators_continue_from_the_result_but_digits_start_fresh() {
        let mut a = app();
        press(&mut a, vec![text("2"), op("+"), text("3"), Action::Equals]);
        press(&mut a, vec![op("×"), text("4")]);
        assert_eq!(a.expr, "ans×4");
        a.apply(Action::Equals);
        assert_eq!(a.result_text, "20");
        a.apply(text("7"));
        assert_eq!(a.expr, "7", "a digit after = starts a new expression");
    }

    #[test]
    fn reciprocal_and_square_apply_to_the_last_value() {
        let mut a = app();
        press(&mut a, vec![text("5"), Action::Equals, op("^(-1)"), Action::Equals]);
        assert_eq!(a.result_text, "0.2");
        press(&mut a, vec![op("²"), Action::Equals]);
        assert_eq!(a.result_text, "0.04");
    }

    #[test]
    fn open_parentheses_are_closed_automatically() {
        let mut a = app();
        press(&mut a, vec![func("sin"), text("30"), Action::Equals]);
        assert_eq!(a.expr, "sin(30)");
        assert_eq!(a.result_text, "0.5");
        press(&mut a, vec![Action::Clear, text("(2+3"), Action::Equals]);
        assert_eq!(a.result_text, "5");
    }

    #[test]
    fn errors_are_shown_and_cleared_on_the_next_edit() {
        let mut a = app();
        press(&mut a, vec![text("1"), op("÷"), text("0"), Action::Equals]);
        assert_eq!(a.error.as_deref(), Some("Cannot divide by zero"));
        assert!(!a.just_evaluated);
        a.apply(Action::Backspace);
        assert_eq!(a.error, None);
        assert_eq!(a.expr, "1÷");
    }

    #[test]
    fn backspace_removes_whole_function_tokens() {
        let mut a = app();
        press(&mut a, vec![text("2"), op("+"), func("asinh"), Action::Backspace]);
        assert_eq!(a.expr, "2+");
        a.apply(Action::Backspace);
        assert_eq!(a.expr, "2");
        press(&mut a, vec![Action::Backspace, Action::Backspace]);
        assert_eq!(a.expr, "");
    }

    #[test]
    fn negate_cases() {
        let cases = [
            ("", "-"),
            ("5", "-5"),
            ("-5", "5"),
            ("2+3", "2+-3"),
            ("2+-3", "2+3"),
            ("2×3.5", "2×-3.5"),
            ("(1+2)", "-((1+2))"),
            ("-((1+2))", "(1+2)"),
            ("sin(30", "sin(-30"), // the trailing number is negated, like the +/- key on Windows
        ];
        for (before, after) in cases {
            let mut a = app();
            a.expr = before.into();
            a.apply(Action::Negate);
            assert_eq!(a.expr, after, "negating {before:?}");
        }
    }

    #[test]
    fn negated_expressions_still_evaluate() {
        for before in ["5", "2+3", "(1+2)", "-4", "2×3.5"] {
            let mut a = app();
            a.expr = before.into();
            a.apply(Action::Negate);
            a.apply(Action::Equals);
            assert_eq!(a.error, None, "negating {before:?} gave {:?}", a.expr);
        }
    }

    #[test]
    fn memory_buttons() {
        let mut a = app();
        press(&mut a, vec![text("6"), Action::MemStore]);
        let m = a.calc.memory().cloned().unwrap();
        assert_eq!(a.calc.format(&m), "6");

        press(&mut a, vec![Action::Clear, Action::MemRecall]);
        assert_eq!(a.expr, "mem");
        a.apply(Action::MemAdd); // 6 + 6
        let m = a.calc.memory().cloned().unwrap();
        assert_eq!(a.calc.format(&m), "12");

        press(&mut a, vec![Action::Clear, text("5"), Action::MemSubtract]);
        let m = a.calc.memory().cloned().unwrap();
        assert_eq!(a.calc.format(&m), "7");

        a.apply(Action::MemClear);
        assert!(a.calc.memory().is_none());
        press(&mut a, vec![Action::Clear, Action::MemRecall]);
        assert_eq!(a.expr, "", "MR with empty memory inserts nothing");
    }

    #[test]
    fn changing_digits_rerenders_the_result() {
        let mut a = app();
        press(&mut a, vec![text("10"), op("÷"), text("3"), Action::Equals]);
        assert_eq!(a.result_text, "3.3333333333333333333333333333333");
        a.set_digits(5);
        assert_eq!(a.result_text, "3.3333");
    }

    #[test]
    fn angle_mode_changes_the_result() {
        let mut a = app();
        press(&mut a, vec![func("sin"), text("90"), Action::Equals]);
        assert_eq!(a.result_text, "1");
        a.calc.set_angle_mode(AngleMode::Radians);
        press(&mut a, vec![Action::Equals]);
        assert_eq!(a.result_text, "0.89399666360055789051826949840421"); // sin(90 rad), checked against mpmath
    }

    /// Every function the keypad can insert must exist in the engine.
    #[test]
    fn every_keypad_function_exists() {
        let calc = Calculator::new();
        let mut names: Vec<String> = ["ncr", "npr", "dms", "deg", "sqrt", "cbrt", "root", "abs", "log", "logb", "ln", "rand"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for base in ["sin", "cos", "tan", "sec", "csc", "cot"] {
            for second in [false, true] {
                for hyper in [false, true] {
                    names.push(trig_name(base, second, hyper));
                }
            }
        }
        for n in names {
            assert!(calc.registry().contains(&n), "keypad inserts unknown function {n}");
        }
    }

    #[test]
    fn long_results_get_smaller_fonts() {
        assert_eq!(fit_size(450.0, 1, 30.0, 13.0), 30.0);
        let long = fit_size(450.0, 33, 30.0, 13.0);
        assert!((13.0..30.0).contains(&long), "{long}");
        assert!(long * 0.62 * 33.0 <= 450.0 + 0.5, "33 digits must fit in 450px");
        assert_eq!(fit_size(450.0, 500, 30.0, 13.0), 13.0, "never below the minimum");
        assert_eq!(fit_size(450.0, 0, 30.0, 13.0), 30.0);
    }

    #[test]
    fn trig_names() {
        assert_eq!(trig_name("sin", false, false), "sin");
        assert_eq!(trig_name("sin", true, false), "asin");
        assert_eq!(trig_name("sin", false, true), "sinh");
        assert_eq!(trig_name("sec", true, true), "asech");
    }

    #[test]
    fn history_keeps_the_typed_expressions() {
        let mut a = app();
        press(&mut a, vec![text("6"), op("×"), text("7"), Action::Equals]);
        let h = a.calc.history();
        assert_eq!((h[0].input.as_str(), h[0].output.as_str()), ("6×7", "42"));
    }

    #[test]
    fn assigned_variables_are_visible_and_usable() {
        let mut a = app();
        a.expr = "r = 2".into();
        a.equals();
        a.expr = "r*3".into();
        a.equals();
        assert_eq!(a.result_text, "6");
        let names: Vec<_> = a.calc.variables().iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(names, ["r"]);
    }

    #[test]
    fn derivative_tab_computes_and_evaluates() {
        let mut a = app();
        a.calc.set_angle_mode(AngleMode::Radians);
        a.deriv.input = "x^2*sin(x)".into();
        a.deriv.point = "0".into();
        a.run_derivative();
        let r = a.deriv.result.clone().unwrap();
        assert_eq!(r.expr, "2*x*sin(x) + x^2*cos(x)");
        let (f, df) = a.deriv.at_point.clone().unwrap().unwrap();
        assert_eq!((f.as_str(), df.as_str()), ("0", "0"));

        a.deriv.point = "pi".into();
        a.refresh_point();
        let (_, df) = a.deriv.at_point.clone().unwrap().unwrap();
        // d/dx x^2 sin x at pi = -pi^2
        assert!(df.starts_with("-9.8696044010893586188344909998"), "{df}");
        // the session is untouched
        assert!(a.calc.history().is_empty());
    }

    #[test]
    fn derivative_tab_reports_errors_and_moves_results() {
        let mut a = app();
        a.deriv.input = "x!".into();
        a.run_derivative();
        assert!(a.deriv.result.is_none());
        assert!(a.deriv.error.as_deref().unwrap().contains("factorial"));

        a.deriv.input = "x^3".into();
        a.deriv.var = "x".into();
        a.run_derivative();
        assert!(a.deriv.error.is_none());
        a.deriv.point = "x".into(); // the variable is not a valid point
        a.refresh_point();
        assert!(a.deriv.at_point.clone().unwrap().is_err());

        a.derivative_to_calculator();
        assert_eq!(a.tab, Tab::Calculator);
        assert_eq!(a.expr, "3*x^2");
    }

    #[test]
    fn variable_form_logic() {
        let mut a = app();
        // the same calls the form makes
        assert!(a.calc.set_variable("r", "2*2.5").is_ok());
        a.expr = "r*2".into();
        a.equals();
        assert_eq!(a.result_text, "10");
        assert!(a.calc.set_variable("sin", "1").is_err());
        assert!(a.calc.remove_variable("r"));
        a.expr = "r*2".into();
        a.equals();
        assert!(a.error.is_some());
    }

    #[test]
    fn programmer_tab_evaluates_and_syncs_the_bases() {
        let mut a = app();
        a.calc.set_variable("a", "12").unwrap();
        a.prog.input = "(a << 4) | 0xF".into();
        a.run_programmer();
        assert_eq!(a.prog.error, None);
        assert_eq!(
            a.prog.fields,
            ["CF".to_string(), "207".into(), "317".into(), "1100 1111".into()]
        );

        // typing into the HEX box updates the others but not the box itself
        a.prog.fields[0] = "ff".into();
        a.prog.edited(0);
        assert_eq!(a.prog.fields[0], "ff");
        assert_eq!(a.prog.fields[1], "255");
        assert_eq!(a.prog.fields[3], "1111 1111");

        // an invalid digit is reported and leaves the value alone
        a.prog.fields[2] = "9".into();
        a.prog.edited(2);
        assert!(a.prog.field_errors[2].is_some());
        assert_eq!(a.prog.value.unsigned(), 255);
    }

    #[test]
    fn programmer_tab_word_size_and_errors() {
        let mut a = app();
        a.prog.input = "-1".into();
        a.run_programmer();
        assert_eq!(a.prog.fields[0], "FFFF FFFF FFFF FFFF");
        a.prog.set_word(WordSize::Byte);
        assert_eq!(a.prog.fields[0], "FF");
        assert_eq!(a.prog.fields[1], "-1");

        a.calc.set_variable("h", "2.5").unwrap();
        a.prog.input = "h + 1".into();
        a.run_programmer();
        assert!(a.prog.error.as_deref().unwrap().contains("not a whole number"));
        // the shown value is unchanged by a failed evaluation
        assert_eq!(a.prog.fields[1], "-1");

        a.prog.input = "mask = 0xF0 >> 4".into();
        a.run_programmer();
        assert_eq!(a.prog.error, None);
        assert_eq!(a.prog.assigned.as_deref(), Some("mask"));
        assert_eq!(a.calc.eval_str("mask").unwrap(), "-1"); // 0xF0 is -16 in 8 bits; >> is arithmetic
    }

    #[test]
    fn programmer_buttons_build_an_expression() {
        let mut a = app();
        a.calc.set_variable("a", "12").unwrap();
        for (_, _, insert, _) in OPERATOR_KEYS {
            assert!(!insert.is_empty());
        }
        a.prog_insert("(");
        a.prog_insert("a");
        a.prog_insert(" << ");
        a.prog_insert("4");
        a.prog_insert(")");
        a.prog_insert(" | ");
        a.prog_insert("0xF");
        assert_eq!(a.prog.input, "(a << 4) | 0xF");
        assert!(a.prog.focus_input);
        a.run_programmer();
        assert_eq!(a.prog.fields[0], "CF");
        // every button text is valid at the right place
        for text in ["a & a", "a | a", "a ^ a", "~a", "a nand a", "a nor a", "a % a",
                     "a << a", "a >> a", "a >>> a", "rol(a, a)", "ror(a, a)", "(a)"] {
            a.prog.input = text.into();
            a.run_programmer();
            assert_eq!(a.prog.error, None, "{text}");
        }
    }

    #[test]
    fn functions_are_defined_from_the_main_field_and_used() {
        let mut a = app();
        press(&mut a, vec![text("f"), text("("), text("x"), text(")"), text("="), text("x"), op("^"), text("2")]);
        a.equals();
        assert!(a.error.is_none(), "{:?}", a.error);
        assert!(a.result_text.starts_with("defined f(x)"));
        assert_eq!(a.calc.functions().len(), 1);
        assert!(a.calc.history().is_empty());
        a.expr = "f(5)".into();
        a.equals();
        assert_eq!(a.result_text, "25");
        a.expr = "f(x) = ".into();
        a.equals();
        assert!(a.error.is_some());
    }

    #[test]
    fn graph_tab_defaults_and_limits() {
        let mut g = graph::GraphState::new();
        assert_eq!(g.curves.len(), 1);
        for _ in 0..20 {
            g.add_curve("x");
        }
        assert_eq!(g.curves.len(), graph::MAX_CURVES);
        // colours rotate through the palette
        assert_ne!(g.curves[0].color, g.curves[1].color);
    }
}
