use the_dispatch::define;

#[derive(Debug)]
enum CalcInput {
  Add(i64, i64),
  Sub(i64, i64),
  Mul(i64, i64),
  Div(i64, i64),
}

#[derive(Default)]
struct CalcCtx {
  notes: Vec<String>,
}

impl CalcCtx {
  fn note(&mut self, msg: &str) {
    self.notes.push(msg.to_string());
  }
}

define! {
  Calculator {
    parse: String => Option<CalcInput>,
    add: (i64, i64) => i64,
    sub: (i64, i64) => i64,
    mul: (i64, i64) => i64,
    div: (i64, i64) => i64,
  }
}

fn parse_input(input: &str) -> Option<CalcInput> {
  let mut parts = input.split_whitespace();
  let left = parts.next()?.parse::<i64>().ok()?;
  let op = parts.next()?;
  let right = parts.next()?.parse::<i64>().ok()?;
  if parts.next().is_some() {
    return None;
  }

  match op {
    "+" => Some(CalcInput::Add(left, right)),
    "-" => Some(CalcInput::Sub(left, right)),
    "*" => Some(CalcInput::Mul(left, right)),
    "/" => Some(CalcInput::Div(left, right)),
    _ => None,
  }
}

fn run_line<Ctx>(dispatch: &impl CalculatorApi<Ctx>, ctx: &mut Ctx, line: &str) -> Option<i64> {
  let input = dispatch.parse(ctx, line.to_string())?;
  let result = match input {
    CalcInput::Add(a, b) => dispatch.add(ctx, (a, b)),
    CalcInput::Sub(a, b) => dispatch.sub(ctx, (a, b)),
    CalcInput::Mul(a, b) => dispatch.mul(ctx, (a, b)),
    CalcInput::Div(a, b) => dispatch.div(ctx, (a, b)),
  };

  Some(result)
}

fn main() {
  let dispatch = CalculatorDispatch::<CalcCtx, _, _, _, _, _>::new()
    .with_parse(|_ctx: &mut CalcCtx, input: String| parse_input(&input))
    .with_add(|_ctx: &mut CalcCtx, (a, b): (i64, i64)| a + b)
    .with_sub(|_ctx: &mut CalcCtx, (a, b): (i64, i64)| a - b)
    .with_mul(|_ctx: &mut CalcCtx, (a, b): (i64, i64)| a * b)
    .with_div(|ctx: &mut CalcCtx, (a, b): (i64, i64)| {
      if b == 0 {
        ctx.note("divide by zero -> returning 0");
        0
      } else {
        a / b
      }
    });

  let mut ctx = CalcCtx::default();
  let inputs = ["1 + 2", "6 / 0", "3 * 4", "9 - 1", "bad input"];

  for line in inputs {
    match run_line(&dispatch, &mut ctx, line) {
      Some(result) => println!("{line} = {result}"),
      None => println!("{line} = error"),
    }
  }

  if !ctx.notes.is_empty() {
    println!("notes: {:?}", ctx.notes);
  }

  let dispatch =
    dispatch.with_add(|ctx: &mut CalcCtx, (a, b): (i64, i64)| {
      ctx.note("custom add -> adding 100");
      a + b + 100
    });

  let mut custom_ctx = CalcCtx::default();
  let line = "2 + 2";
  let result = run_line(&dispatch, &mut custom_ctx, line).unwrap_or(0);
  println!("custom: {line} = {result}");
  println!("custom notes: {:?}", custom_ctx.notes);
}
