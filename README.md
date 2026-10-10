# scicalc

An arbitrary-precision **scientific calculator written in Rust**, with a
command-line / REPL front end and a desktop GUI. It covers the feature set of
the scientific mode of the Windows Calculator (and adds a few things), and it
is built so that statistics, regression, matrices or other "analysis"
features can be added later without touching the existing code.

```
$ scicalc "0.1 + 0.2"
0.3
$ scicalc "sin(30) + 2^10"
1024.5
$ scicalc -d 50 "sqrt(2)"
1.4142135623730950488016887242096980785696718753769
$ scicalc "ncr(52, 5)"
2598960
```

- [Quick start](#quick-start)
- [Requirements](#requirements)
- [Building](#building)
- [Using the command-line app](#using-the-command-line-app)
- [Using the GUI](#using-the-gui)
- [Expression reference](#expression-reference)
- [Function reference](#function-reference)
- [Precision, rounding and display](#precision-rounding-and-display)
- [Errors](#errors)
- [Compared with the Windows Calculator](#compared-with-the-windows-calculator)
- [Project layout](#project-layout)
- [Extending the calculator](#extending-the-calculator)
- [Testing](#testing)
- [Known limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## Quick start

```sh
# 1. install Rust (once): https://rustup.rs
# 2. from the project folder:
cargo run --release -p calc-cli -- "sin(30) + 2^10"    # one calculation
cargo run --release -p calc-cli                        # interactive session
cargo run --release -p calc-gui                        # desktop GUI
```

The first build downloads and compiles the dependencies; the GUI is by far the
larger part of that. Later builds are incremental.

## Requirements

| Need | Notes |
|---|---|
| Rust **1.95 or newer** | Install with [rustup](https://rustup.rs). 1.95 is the minimum required by the GUI toolkit (`eframe 0.36`). Developed and tested with Rust 1.97.0. |
| A C linker | Normally already there: `cc` on Linux/macOS (`build-essential` / Xcode command line tools), the *MSVC Build Tools* on Windows. No C *libraries* are needed: the number library ([`astro-float`](https://crates.io/crates/astro-float)) is pure Rust. |
| For the GUI on Linux | An OpenGL driver plus X11 or Wayland client libraries at run time. These are present on any normal desktop install. On the Ubuntu 24.04 machine used for testing the GUI ran with `libgl1`, `libegl1`, `libxkbcommon0`, `libxkbcommon-x11-0`, `libwayland-client0`, `libx11-6` and Mesa installed, and no `-dev` packages were needed to build it. |

Platforms: everything was built and tested on **Linux x86-64**. Windows and
macOS are expected to work because all dependencies are cross-platform, but
that has **not** been tried.

## Building

All commands are run from the project folder (the one containing this README).

```sh
cargo build --release              # build everything
cargo build --release -p calc-cli  # only the command-line app
cargo build --release -p calc-gui  # only the GUI
```

The programs end up in `target/release/`:

| Program | Linux / macOS | Windows |
|---|---|---|
| Command line / REPL | `target/release/scicalc` | `target\release\scicalc.exe` |
| Desktop GUI | `target/release/scicalc-gui` | `target\release\scicalc-gui.exe` |

You can copy these files anywhere; they have no data files. To put them on
your `PATH` instead:

```sh
cargo install --path calc-cli    # installs `scicalc`
cargo install --path calc-gui    # installs `scicalc-gui`
```

(`cargo install` puts them in `~/.cargo/bin`, which rustup already adds to your
`PATH`.)

A compiled program only runs on the operating system and CPU type it was built
for. To distribute to other platforms, build on each one (a CI matrix with a
Windows, macOS and Linux runner is the usual way). The core engine has no
OS-specific code and could also be compiled to WebAssembly.

Other useful commands:

```sh
cargo test --workspace                       # run all tests
cargo clippy --workspace --all-targets       # lints
cargo doc --workspace --no-deps --open       # API documentation
```

## Using the command-line app

```
scicalc [OPTIONS] [EXPRESSION]...

  -a, --angle <MODE>    angle mode: deg (default), rad or grad
  -d, --digits <N>      significant digits to show, 1-60 (default 32)
  -f, --functions       list all functions and exit
  -h, --help            show help
  -V, --version         show the version
  --                    treat everything after it as the expression
```

It has three modes:

**One-shot**: give an expression and it prints the result and exits. The exit
status is 1 if the expression is invalid.

```sh
scicalc "2+3*4"                    # 14
scicalc -a rad "sin(pi/6)"         # 0.5
scicalc --angle=grad "sin(100)"    # 1
scicalc -d 5 pi                    # 3.1416
scicalc -- -2^2                    # -4   (use -- if the expression starts with '-')
scicalc "1/0"                      # Cannot divide by zero   (on stderr, exit status 1)
```

> **Quote the expression.** `*`, `(`, `!` and `^` mean things to your shell.
> Use double quotes on Windows (`cmd.exe` treats an unquoted `^` as its escape
> character) and single quotes in bash/zsh if an expression contains `!`
> (`scicalc '10!'`).

**Piped**: with no expression and stdin redirected, every line is evaluated and
its result printed. Empty lines and lines starting with `#` are skipped, and
variables, memory and `:` commands carry over between lines.

```sh
$ printf '# area of a circle\nr = 2.5\npi * r^2\n' | scicalc
2.5
19.634954084936207740391521145497
```

**Interactive (REPL)**: with no expression and a terminal on stdin. Line
editing and history (arrow keys) come from
[`rustyline`](https://crates.io/crates/rustyline). The prompt shows the angle
mode.

```
$ scicalc
scicalc 0.1.0 - scientific calculator. Type :help for help, :quit to leave.
deg> 2+3*4
= 14
deg> sin(30)
= 0.5
deg> :rad
angle mode: radians
rad> x = 7
= 7
rad> x^2
= 49
rad> :ms
memory = 49
rad> :m+ 3
memory = 52
rad> mem * 2
= 104
rad> 1/0
Cannot divide by zero
rad> :quit
```

Commands start with `:` (`quit`, `exit` and `help` also work bare):

| Command | Effect |
|---|---|
| `:deg` `:rad` `:grad` | set the angle mode (`:angle` shows it) |
| `:digits [N]` | show or set the significant digits (1-60) |
| `:ms [expr]` | memory store: the expression, or the last result |
| `:m+ [expr]` / `:m- [expr]` | add to / subtract from memory |
| `:mr` / `:mc` | show / clear memory |
| `:history` | list this session's results |
| `:vars` / `:clear` | list / forget variables and user-defined functions |
| `:funcs [text]` | list functions, optionally filtered |
| `:help` / `:quit` | help / leave (Ctrl-D also leaves) |

## Using the GUI

Start `scicalc-gui`. The window has the calculator on the left and the
history on the right.

**Entering an expression.** Type into the expression line, or press buttons;
both build the same text, and you can mix them. Press **Enter** or **=** to
evaluate. **Esc** clears. Buttons always append at the end of the line.

- Open parentheses are closed automatically when you press `=`.
- After `=`, an *operator* button (`+ − × ÷ x² x^y n! mod 1/x`) continues from
  the result (`ans`); a *digit or function* button starts a fresh expression.
- Anything the keyboard can type works, including `sin(30)+2^10`, `10!`,
  `x = 5`, `ans*2`.

**Top row:** `DEG` / `RAD` / `GRAD` selects the angle mode. `Digits` sets how
many significant digits are shown (1-60); the current result is re-rendered
immediately. When memory is in use, `M = value` is shown on the right.

**Memory row:** `MC` clear, `MR` inserts `mem` into the expression, `M+` / `M−`
add / subtract the value of the current expression (or last result), `MS`
stores it. Memory is a single register.

**Trigonometry row:** `sin cos tan sec csc cot`. Toggle **2nd** for the inverse
functions (`asin acos ...`), **hyp** for the hyperbolic ones (`sinh cosh ...`),
or both (`asinh ...`). The button labels change to show what you will get.

**Extras:** `nCr`, `nPr`, `dms` (decimal degrees to d.mmss), `deg` (d.mmss to
decimal degrees; this is a *function*, unrelated to the DEG angle mode), and
`rand` (inserts `rand()`).

**Keypad** (same arrangement as the Windows scientific mode):

| | | | | |
|---|---|---|---|---|
| `2nd` | `π` | `e` | `C` clear | `Bksp` |
| `x²` / `x³` | `1/x` | `\|x\|` | `mod` | `n!` |
| `√x` / `³√x` | `(` | `)` | `x^y` / `y√x` | `÷` |
| `10^x` / `2^x` | `7` | `8` | `9` | `×` |
| `log` / `logb` | `4` | `5` | `6` | `−` |
| `ln` / `e^x` | `1` | `2` | `3` | `+` |
| `,` | `±` | `0` | `.` | `=` |

(`a / b` means *normal* / *with 2nd on*.) `1/x` appends `^(-1)`, `x²` appends
`²`, `±` flips the sign of the trailing number, `Bksp` deletes the last
character (or a whole `name(`), and `,` separates function arguments, e.g.
`root(27, 3)` or `ncr(5, 2)`.

**History panel:** newest first. Click an entry to put its expression back on
the line. `Clear` empties it. The result text in the display is selectable, so it can be copied.

Long results shrink to fit; hover over the display to see the full text.

**Variables panel:** the form at the top defines variables (see *Expression
reference*); the list below shows them. Click one to insert its name (into the
expression field, or the function field on the Derivative tab).

**Functions panel:** lists the functions you defined (`f(x) = ...`). Click one
to insert a call (or, on the Graph tab, to plot it); `x` removes it.

### Graph tab

The **Graph** tab plots `y = f(x)` for up to eight functions at once.

- Each row is a formula in `x`: `sin(x)/x`, `x^3 - 2x`, or a call such as
  `f(x)`. A row can also be a *definition*, `g(t) = t^2/20`: it is plotted
  (its first parameter is the variable) and, when you leave the field or press
  Enter, stored so other rows and the calculator can use `g(...)`.
- The coloured square changes the colour, the check box shows or hides the
  curve, `x` removes it. Errors (unknown names, wrong argument counts) are
  shown in red under the row.
- **Drag** to pan, **scroll wheel** (or pinch) to zoom around the pointer,
  **double-click** or `Reset view` to return to the default view (x about
  -9 to 9, equal scale on both axes). `Fit y` scales the vertical axis to the
  curves in the current x range, ignoring the most extreme 2 % of values so
  poles do not squash everything else.
- Hovering draws a cursor line, dots on every curve and a read-out box with
  the value of each function at the pointer's x; the line under the plot shows
  the pointer's coordinates.
- The angle mode (`DEG` / `RAD` / `GRAD`) applies to trigonometric functions,
  exactly as in the calculator. **In `DEG` mode `sin(x)` is nearly flat over
  -9..9**, because those are degrees; switch to `RAD` for the familiar wave.
- Other variables are read when the plot is drawn, so changing a variable in
  the Variables panel changes the curve immediately.
- Gaps are drawn where a function is undefined (`sqrt(x)` for `x < 0`,
  `ln(x)`) and the line is broken at poles (`tan(x)`, `1/x`) instead of
  joining the two branches. A removable hole such as `sin(x)/x` at `x = 0`
  shows as a one-pixel gap.

Not included: parametric curves, polar curves, implicit equations (`x^2 + y^2 =
1`) and 3D plots. Zeros, extrema, tangent lines and areas are not marked
either. The curve code is separate from the drawing code so these can be added.

### Programmer tab

The **Programmer** tab works on fixed-width integers, like the programmer mode
of the Windows calculator.

- **Expression line:** type e.g. `(a << 4) | 0xF` and press Enter (or
  *Evaluate*). Variables from the Variables panel can be used (click one to
  insert its name). `name = expression` stores the result back as a variable
  (as a signed number).
- **Operator buttons:** two rows under the expression line, each with its
  name above it (AND, OR, XOR, NOT, NAND, NOR, MOD, LSHIFT, RSHIFT = arithmetic,
  URSHIFT = logical, ROL, ROR, and parentheses). A button appends its operator to
  the expression and puts the cursor back in the line, so you can keep typing;
  hover a button for a one-line description.
- **Bits:** 8, 16, 32 or 64. Widening keeps the signed value, narrowing keeps
  the low bits.
- **Four bases:** the HEX, DEC, OCT and BIN boxes always show the current value.
  Type into any box and the others follow (spaces and `_` are ignored; only DEC
  accepts a minus sign). DEC is signed; a line below shows the unsigned value
  when it differs.
- **Bit grid:** every bit is shown, most significant first, 16 per row in groups
  of four, with the index of the row's top bit on the left. Click a bit to flip
  it.
- **Errors:** shown in red under the expression. A variable that holds a
  non-integer (e.g. `2.5`), a decimal literal, a number that does not fit the
  chosen bit size, or division by zero is rejected.

Operators, from lowest to highest precedence (the same order as C):

| operators | meaning |
|---|---|
| `\|` `nor` | OR, NOR |
| `^` `xor` | XOR |
| `&` `nand` | AND, NAND |
| `<<` `>>` `>>>` | shift left, arithmetic shift right (keeps the sign), logical shift right |
| `+` `-` | add, subtract |
| `*` `/` `%` `mod` | multiply, truncating divide, remainder (sign of the dividend) |
| unary `-` `~` | negate, bitwise NOT |

Functions: `rol(x, n)` and `ror(x, n)` rotate within the chosen bit size.
Literals: `255`, `0xFF`, `0b1010`, `0o17`, with optional `_` separators
(`0xFF_FF`). Plain digits are always decimal, so `ff` is a variable name, not a
number.

Arithmetic **wraps around** like hardware: in 8 bits `0x7F + 1` is `-128`. A
shift by the bit size or more gives `0` (or all ones for an arithmetic shift of a
negative number); a negative shift or rotate count is an error. Note that `^`
is XOR here, while it is "power" on the Calculator tab.

Not included: rotate through carry (it needs a carry flag that persists between
operations) and the Windows keypad. I could not compare against the real Windows
app, so edge cases such as large shift counts follow the rules above and may
differ from it.

### Derivative tab

The **Derivative** tab (next to *Calculator*, top left) differentiates a
function symbolically:

1. Type a function, e.g. `x^2*sin(x)`, and press Enter.
2. Choose the variable (default `x`) and the order (1 to 10); changing either
   recomputes.
3. The result is shown typeset (fractions, roots, raised exponents), with the
   plain-text form below it, e.g. `2*x*sin(x) + x^2*cos(x)`. The text form is
   selectable and is what *Copy to calculator* uses. Very wide results scroll
   sideways.
4. *Evaluate at* takes a number or expression (`pi/4`, `2.5`) and shows
   `f(x0)` and `f'(x0)` at full precision.
5. *Copy to calculator* moves the result to the Calculator tab; *Use as
   function* puts it into the function field.

Things to know:

- **Angle mode matters.** In DEG the derivative of `sin(x)` is `cos(x)*pi/180`
  because `sin(x)` means sin of x degrees. Switch to RAD for the textbook
  forms. Changing the mode recomputes the result.
- **Other names are constants:** `pi`, `e`, variables from the Variables panel,
  and any other letter. `t^2*x` differentiated by `x` is `t^2`.
- **Piecewise functions** (`abs`, `sign`, `floor`, `ceil`, `trunc`, `round`,
  `frac`, `mod`) are differentiated where they are smooth; a note tells you the
  result is invalid at their jumps and corners.
- **Not supported:** `x!`, `min`, `max`, `ncr`, `npr`, `dms`, `deg` of the
  variable. These give an error instead of a wrong answer.
- **Simplification is local.** Constants are folded exactly, like terms are
  combined, and `x/x` becomes `1` (ignoring `x = 0`), but the result is not
  always the shortest form a person would write.
- Domain of validity is the usual one: for example the derivative of
  `sqrt(x)` is only meaningful for `x > 0`, and `sqrt(u)^2` is simplified to
  `u`, which assumes `u >= 0`.
- **Typesetting** uses the [RaTeX](https://github.com/erweixin/RaTeX) library
  (pure Rust, KaTeX-compatible, with its fonts embedded). The formula is
  rendered to an image at the screen's pixel density and cached. It adds about
  4 MB to the GUI binary and noticeably to a clean build. If a formula cannot
  be typeset, the plain text is shown instead.

The same engine is available as `Calculator::differentiate(input, var, order)`.

## Expression reference

Everything is **case-insensitive** (`SIN(30)`, `Pi`).

**User-defined functions:** `f(x) = x^2 + 1`, `g(a, b) = a*f(b)`. Then
`f(3)` is 10 and `g(2, 3)` is 20. Rules:

- Parameters shadow variables of the same name; any other name in the body is
  a variable that is looked up *when the function is called*, so changing a
  variable changes the function.
- Functions can call other functions; redefining one updates the others.
- **No recursion**: a function cannot call itself, directly or through
  another function (rejected at definition time).
- Built-in names, `pi`, `e`, `ans`, `mem` and existing variables cannot be
  used as function names, and a function name cannot be reused as a variable.
- A call must have exactly as many arguments as the definition has parameters.
- Implementation note: a call is replaced by the function's body with the
  arguments substituted ("inlining"), so derivatives and plots work through
  user functions with no extra code. Expansion is limited in depth and size
  (`d(d(d(...)))` with large bodies is rejected, not evaluated).
- The GUI main field and graph rows, and the CLI, accept definitions. In the
  CLI they last for the session (`:vars` lists them).

**Numbers:** `42`, `3.14`, `.5`, `5.`, and scientific notation `1.5e-3`,
`2E+10`. A result printed by the calculator can be typed back in unchanged.

**Operators**, from lowest to highest precedence:

| Operators | Meaning | Associativity |
|---|---|---|
| `+` `-` | add, subtract | left |
| `*` `/` `mod` `%`, and *implicit multiplication* | multiply, divide, remainder | left |
| unary `-` `+` | sign | prefix |
| `^` `**` | power | **right** (`2^3^2` is 512) |
| `!` `²` `³` | factorial, square, cube | postfix |

Consequences worth knowing:

- `-2^2` is **−4** (the power binds tighter than the sign); use `(-2)^2` for 4.
- `2^3!` is `2^(3!)` = 64; `-3!` is `-(3!)` = −6.
- `mod` (and its alias `%`) gives the remainder with the sign of the dividend:
  `-7 mod 3` is −1, `7.5 mod 2` is 1.5. **`%` is not "percent".**
- Implicit multiplication works between a value and a following `(`, name or
  function: `2pi`, `3(4+5)`, `(1+2)(3+4)`, `2 sin(30)`, `x(y+1)`. Two numbers
  next to each other (`2 3`) are an error.

**Symbols you can type instead of ASCII:** `×` `·` (multiply), `÷`, `−`,
`√(x)` (= `sqrt(x)`), `π`, `x²`, `x³`.

**Constants:** `pi` (or `π`), `e`.

**Special names:** `ans` is the last result, `mem` is the memory register
(0 when empty).

**Variables:** `name = expression` defines one, e.g. `r = 2.5`, then
`pi*r^2`. Names are letters, digits and `_`, starting with a letter or `_`.
`pi`, `e`, `ans`, `mem`, `mod` and every function name are reserved. A failed
statement changes nothing (not the variable, not `ans`).
In the GUI you can also use the form at the top of the **Variables** panel:
type the name, press Tab, type the value (any expression, such as `2*pi` or
`r^2`), and press Enter or click `Set`. Focus then returns to the name box for
the next variable. Errors appear under the form and change nothing. Each listed
variable has `edit` (load it into the form) and `x` (remove it) buttons;
clicking its name inserts it into the expression, and `Clear` forgets them all.
Defining a variable this way does not touch `ans` or the history.

**Functions** are called as `name(args)`; the parentheses are required
(`sin 30` is an error with a hint). Arguments are separated by commas.

## Function reference

Run `scicalc -f` (or `:funcs` in the REPL) for the live list; there are 50.

| Group | Functions |
|---|---|
| Roots, powers, exponentials, logarithms | `sqrt(x)` `cbrt(x)` `root(x, n)` `exp(x)` `ln(x)` `log(x)` (base 10) `log10(x)` `log2(x)` `logb(x, b)` |
| Rounding and sign | `abs` `floor` `ceil` `trunc` (towards zero) `frac` (fractional part) `round` (half away from zero) `sign` |
| Trigonometry (see [angle modes](#angle-modes)) | `sin` `cos` `tan` `sec` `csc` `cot` and inverses `asin` `acos` `atan` `asec` `acsc` `acot` |
| Hyperbolic | `sinh` `cosh` `tanh` `sech` `csch` `coth` and inverses `asinh` `acosh` `atanh` `asech` `acsch` `acoth` |
| Combinatorics | `fact(n)` (same as `n!`), `ncr(n, k)` combinations, `npr(n, k)` permutations |
| Angle notation | `dms(x)` decimal degrees to `d.mmss` (`dms(10.5)` is `10.3`), `deg(x)` the reverse (`deg(10.3)` is `10.5`) |
| Several arguments | `sum(...)` `mean(...)` `min(...)` `max(...)`: one or more arguments |
| Random | `rand()`: uniform in [0, 1) (not cryptographically secure) |

Domain notes: `root(x, n)` accepts a negative `x` only for odd integer `n`
(`root(-27, 3)` is −3); `x^y` with a negative base needs an integer `y`;
`n!` needs an integer from 0 to 10000; `acot` has the range (0, π).

### Angle modes

Trigonometric functions read their argument, and inverse trigonometric
functions return their result, in the current mode: **degrees** (the default),
**radians** or **gradians** (400 per turn). Hyperbolic functions are not
affected. Switch with `:deg` / `:rad` / `:grad`, `--angle`, or the GUI buttons.

At exact angles the result is exact: `sin(30)` is `0.5`, `cos(90)` is `0`,
`tan(45)` is `1`, `sin(pi)` is `0` in radians. Where a function does not exist,
you get an error rather than a huge number: `tan(90)`, `cot(0)`, `sec(270)`.

## Precision, rounding and display

Numbers are binary floating point with a **256-bit mantissa (about 77 decimal
digits)**, using [`astro-float`](https://crates.io/crates/astro-float). By
default **32 significant digits are shown** (settable from 1 to 60), rounded
from the full internal value, so ordinary decimal arithmetic looks the way it
does on paper: `0.1+0.2` is `0.3`, `1/3*3` is `1`, `4.35*100` is `435`.

Plain notation is used for exponents from −6 up to the digit setting; outside
that, scientific notation (`1e-7`, `1.5e-10`, `2.5e+40`). Decimal exponents up to
about ±646 million are supported (`1e5000*1e5000` is `1e+10000`). Literals
beyond that range are rejected, and results that fall below it report
`Underflow` instead of silently becoming 0.

Because inputs like `0.7` are not exactly representable in binary, three rules
make the results behave like exact decimal arithmetic where that matters:

1. **Cancellation noise becomes zero.** A sum or difference smaller than about
   `2^-248` (3·10⁻⁷⁵) of its operands is rounding noise, so `0.3 - 0.1*3`,
   `sqrt(2)^2 - 2` and `0.294*102000 - 29988` are exactly `0`.
2. **Near-integers count as integers** in operations that care about
   integrality: `floor`, `ceil`, `trunc`, `frac`, `round`, `mod`, `n!`, `ncr`,
   `npr`, `sqrt`, `cbrt`, `root` and powers. So `floor(0.294*102000)` is
   `29988` and `(0.3 mod 0.1)` is `0`. "Near" means within about 256 units in
   the last place; real differences far smaller than anything visible on screen
   are still preserved (`floor(3 - 1e-60)` is `2`).
3. **Exact special cases are exact:** integer powers (while the result fits in
   77 digits), square and cube roots of perfect powers, `x^(1/n)`, `x^0.5` and
   other binary fractions, `log10`/`log2`/`logb` of exact powers
   (`floor(log10(1000))` is `3`), and trigonometry at multiples of 90°.

The flip side: differences below the noise level really are invisible
(`(1+1e-80) - 1` is `0`), and a result can only be as accurate as ~77 digits.
A trigonometric argument that is both huge (above about 3·10³⁸) **and** had to
be rounded to be stored (`sin(1e200)`) gives an error instead of an answer,
because its sine would describe a different number. Huge arguments that are
exactly representable (`sin(1e100)`) are computed correctly.

## Errors

Errors are reported in words; nothing is silently replaced by `NaN` or
infinity.

| Message | Cause |
|---|---|
| `Cannot divide by zero` | `1/0`, `5 mod 0`, `0^-1` |
| `Result is undefined: ...` | `0/0`, `0^0`, `tan(90)`, `cot(0)` |
| `Invalid input: ...` | `sqrt(-1)`, `ln(0)`, `asin(2)`, `(-8)^(1/3)`, `2.5!`, `acosh(0.5)` ... |
| `Overflow: the result is too large` | `exp(1e20)`, `10^(10^10)`, `1e650000000` |
| `Underflow: the result is too small to represent` | `exp(-1e10)`, `1e-650000000`, `(1e-640000000)^2` |
| `Syntax error: ...` | with the position of the problem, e.g. `unexpected '*' at position 5` |
| `Unknown name 'x'` / `Unknown function 'f'` | undefined variable or function |
| `f() expects N argument(s), got M` | wrong number of arguments |
| `Cannot assign: 'pi' is a reserved name` | assignment to a reserved name |

## Compared with the Windows Calculator

I wrote the target feature list from my knowledge of the Windows Calculator's
scientific mode. **I have not compared this program's output with the real
Windows Calculator**, so identical results in every last digit are not claimed.

Implemented: all four arithmetic operations with precedence and parentheses;
`x²` `x³` `x^y` `√x` `³√x` `y√x` `10^x` `2^x` `e^x` `log` `ln` `logᵧx`; `n!`;
`1/x`; `|x|`; `mod`; `π` and `e`; `±`; sin/cos/tan, sec/csc/cot, their inverses
and all twelve hyperbolic variants; degrees/radians/gradians; `dms` and `deg`;
`MC MR M+ M− MS`; a history list; scientific-notation input; the 2nd and hyp
toggles. Extras beyond that: `nCr`, `nPr`, `rand`, `sum/mean/min/max`,
`round/trunc/frac/sign/floor/ceil`, `log2`, variables, `ans`, adjustable digits.

Different or missing:

- **Expression entry**, not immediate execution: you build a formula and
  evaluate it, rather than the display changing after each button. This is
  what makes later extensions (functions, variables, scripts) straightforward.
- `-2^2` is −4 here (ordinary mathematical convention).
- `n!` is defined only for integers 0-10000; there is no gamma function for
  non-integers.
- Graphing is limited to `y = f(x)` curves (see the Graph tab). Not implemented:
  parametric, polar, implicit and 3D plots, unit and currency converters, date calculation, complex numbers.
- No `F-E` or `Exp` key: type scientific notation directly (`1.5e3`).
- Memory is one register.
- The number engine is `astro-float` with the rules above, not Microsoft's.

## Project layout

```
scicalc/
├── Cargo.toml          workspace
├── calc-core/          the engine; no UI and no I/O (a library)
│   ├── src/
│   │   ├── lexer.rs        text -> tokens
│   │   ├── parser.rs       tokens -> syntax tree (Pratt parser)
│   │   ├── eval.rs         walks the tree
│   │   ├── ops.rs          all math, with domain checks and error mapping
│   │   ├── num.rs          Num type, angle modes, decimal rendering
│   │   ├── functions.rs    the function registry and all built-in functions
│   │   ├── programmer.rs   fixed-width integer mode: bitwise ops, bases, bit access
│   │   ├── symbolic.rs     symbolic differentiation, simplification, LaTeX output
│   │   ├── userfn.rs       user-defined functions: parsing of definitions, inlining
│   │   ├── fasteval.rs     fast f64 evaluator used for plotting
│   │   ├── calculator.rs   a session: memory, history, variables, settings
│   │   ├── error.rs        CalcError
│   │   └── lib.rs
│   └── tests/              engine.rs, derivative.rs, programmer.rs, userfn.rs, plotfn.rs
├── calc-cli/           `scicalc`: one-shot, piped and REPL front end
└── calc-gui/           `scicalc-gui`: egui / eframe desktop front end
    └── src/typeset.rs      LaTeX -> image -> texture (RaTeX), cached
    └── src/graph.rs        Graph tab: view/tick/segment math (tested) and drawing
```

A calculation flows through `lexer -> parser -> eval -> ops`, always through
the `Calculator` type, which is the only thing a front end needs:

```rust
use calc_core::Calculator;

let mut calc = Calculator::new();
assert_eq!(calc.eval_str("0.1 + 0.2").unwrap(), "0.3");
```

## Extending the calculator

Every function lives in a registry, and adding one is a single call. This is
how analysis features are meant to be added:

```rust
use calc_core::{Arity, Calculator, Num};

let mut calc = Calculator::new();
calc.registry_mut().register(
    "double", Arity::Exact(1), "twice the argument",
    |ctx, args| ctx.mul(&args[0], &Num::from_i64(2)),
);
assert_eq!(calc.eval_str("double(21)").unwrap(), "42");
```

- `Arity` can be `Exact(n)`, `AtLeast(n)` or `Range(lo, hi)`, so list-style
  functions (`mean(1, 2, 3, ...)`) are supported; look at `sum`/`mean`/`min`/`max`
  in `functions.rs`.
- Inside a function, use the `Ctx` methods (`ctx.add`, `ctx.mul`, `ctx.ln`,
  `ctx.sin`, ...). They validate domains and return proper errors. `Num`
  values convert with `ctx.to_f64(&n)` / `calc.to_f64(&n)` if you need ordinary
  floats (e.g. for plotting).
- Natural next steps are a `register_stats(&mut Registry)` function in a new
  module (mean/median/stdev/regression), matrices, or units, called next to the
  built-ins. Nothing in the parser or evaluator has to change.
- A new front end (a web page through WebAssembly, a TUI, a mobile shell) only
  depends on `calc-core`.

## Testing

```sh
cargo test --workspace
```

- **Engine (50 tests):** precedence and associativity; display and notation;
  every function family; angle modes; errors and their messages; variables,
  memory and history; the registry. Expected values for non-trivial results
  were computed independently with Python's `mpmath` (100+ digits), not taken
  from the program's own output.
- **Decimal exactness:** seeded pseudo-random suites check thousands of
  expressions whose exact decimal result is an integer or a half-integer
  (products, quotients, sums, perfect squares, factorials, ...). These caught
  a real defect (roughly 6–7% of such cases were off by one before the noise rules
  above were added).
- **Derivatives (11 tests):** about 100 functions are differentiated and each
  result is compared with a central finite difference computed independently
  at 256-bit precision (agreement required to 25 digits); this covers every
  rule and built-in, positive and negative arguments, all angle modes, higher
  orders, and awkward parenthesisation. Further tests fix the exact simplified
  form of common cases, the LaTeX output and the error cases.
- **Programmer mode (13 tests):** 400 random expressions in all four bit sizes
  (mixed literal styles, precedence, shifts, rotates, variables) are compared
  with expected values produced by an independent Python implementation of the
  same rules. Further tests cover wrap-around, division, shifts and rotates,
  literal range checks, variables (whole / fits / not whole), assignment,
  formatting and base parsing with round trips.
- **User-defined functions (11 tests):** calls, nesting, simultaneous
  substitution (no argument capture), shadowing, redefinition, the error cases
  (reserved names, recursion, cycles, arity), derivatives through functions,
  and a size-blow-up guard.
- **Plot evaluator (3 tests):** about 60 expressions covering every function
  the plotter supports are evaluated at 120 points each, in all three angle
  modes, with the fast `f64` evaluator and with the 256-bit engine; they must
  agree to 1e-9 relative, and where the engine reports an error the fast
  evaluator must return NaN or infinity.
- **GUI logic (24 tests):** the button-handling code (continuing from `ans`,
  negation, backspace, memory, auto-closing parentheses, 2nd/hyp naming,
  defining functions from the main field).
- **Graph math (13 tests):** tick steps and labels, number formatting, the
  view transform, zoom about the pointer, panning, `Fit y`, and the
  curve-breaking rules (smooth curves stay in one piece; `sqrt`, `1/x` and
  `tan` break where they should; steep curves such as `x^9` do not).
- The GUI, including the Graph tab (adding curves, hover read-out, zoom, pan,
  `Fit y`, double-click reset), was also driven end to end under a virtual X display with real
  keyboard and mouse events (typing, Enter, clicking, 2nd, memory, RAD) and
  inspected via screenshots.

## Known limitations

- **Linux only so far.** Not tried on Windows or macOS. The typeset result was
  checked on a dark theme at 1x and 2x scale; the light theme is untested.
- **Real numbers only.** `sqrt(-1)` is an error, not `i`.
- **Evaluation is synchronous.** The GUI evaluates on its UI thread and there
  is no cancel button. Every expression I tried returned quickly (e.g.
  `10000!`, `exp(1e9)`, `2^(2^30)` in milliseconds; `ncr(1000000, 500000)` in
  under 2 s even in a debug build), but a pathological input could make the
  window unresponsive.
- **A numeric-library quirk is worked around, not fixed.** `astro-float`'s
  general `pow` never returns when the exact result is representable
  (`4^0.5`), so those cases are computed by an exact path in `ops.rs`
  (`pow_dyadic`). If you replace or upgrade the numeric backend, rerun the
  tests under `exact_fractional_powers_do_not_hang_and_stay_exact`.
- **The GUI's buttons append at the end of the line**, not at the cursor, and
  settings and history are not saved between runs.
- **Some math glyphs are missing** from egui's bundled fonts, so button labels
  use `x^y`, `10^x`, `Bksp` etc. instead of `xʸ`, `10ˣ`, `⌫`.
- **No license has been chosen yet.** Add one before sharing the code. The
  dependencies are permissively licensed (`astro-float` is MIT; `eframe`/`egui`
  are MIT or Apache-2.0).

- **Plots are f64 approximations.** The graph evaluates in 64-bit floats for
  speed, so it can differ from the 256-bit calculator in the last digits and
  cannot show detail finer than about 1e-15 relative. Pole detection is a
  heuristic (a break where the curve jumps over half the plot height against
  its previous direction); a very sharp legitimate peak can lose one segment,
  and a pole can be missed if no sample lands near it at extreme zoom levels.
- **The plot is redrawn every frame** with about three samples per pixel per
  curve. It is fast for ordinary functions; with eight heavy curves in a debug
  build it can feel sluggish (use `--release`).

## Troubleshooting

| Problem | Fix |
|---|---|
| `package ... requires rustc 1.95` | `rustup update` |
| `linker 'cc' not found` (Linux) | `sudo apt install build-essential` (or your distribution's equivalent); on Windows install the MSVC Build Tools |
| The GUI window does not open on Linux, or a GL error appears | Make sure an OpenGL driver is installed. In a VM or over a remote display, `LIBGL_ALWAYS_SOFTWARE=1 scicalc-gui` forces software rendering |
| `scicalc: command not found` after `cargo install` | Add `~/.cargo/bin` to `PATH` (`source ~/.cargo/env`) |
| `sin 30` gives a syntax error | Functions need parentheses: `sin(30)` |
| `sin(90)` is not 1 | The angle mode is degrees by default, so `sin(90)` is 1 there; if you switched to radians use `sin(pi/2)`, or `:deg` |
| Shell eats `*`, `!` or `^` | Quote the expression (see [above](#using-the-command-line-app)) |

## License

No license is set yet (see [Known limitations](#known-limitations)).
