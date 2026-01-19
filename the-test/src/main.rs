use std::sync::Arc;

use the_dispatch::{define, DispatchRegistry, DynHandler, DynValue};

#[derive(Clone, Copy, Debug)]
enum Op {
  Add,
  Sub,
  Mul,
  Div,
}

#[derive(Clone, Copy, Debug)]
struct Expr {
  left: i64,
  op: Op,
  right: i64,
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
    parse: String => Option<Expr>,
    add: (i64, i64) => i64,
    sub: (i64, i64) => i64,
    mul: (i64, i64) => i64,
    div: (i64, i64) => i64,
  }
}

fn parse_expr(input: &str) -> Option<Expr> {
  let mut parts = input.split_whitespace();
  let left = parts.next()?.parse::<i64>().ok()?;
  let op = parts.next()?;
  let right = parts.next()?.parse::<i64>().ok()?;
  if parts.next().is_some() {
    return None;
  }

  let op = match op {
    "+" => Op::Add,
    "-" => Op::Sub,
    "*" => Op::Mul,
    "/" => Op::Div,
    _ => return None,
  };

  Some(Expr { left, op, right })
}

fn eval_line<Ctx>(dispatch: &impl CalculatorApi<Ctx>, ctx: &mut Ctx, line: &str) -> Option<i64> {
  let expr = dispatch.parse(ctx, line.to_string())?;
  let result = match expr.op {
    Op::Add => dispatch.add(ctx, (expr.left, expr.right)),
    Op::Sub => dispatch.sub(ctx, (expr.left, expr.right)),
    Op::Mul => dispatch.mul(ctx, (expr.left, expr.right)),
    Op::Div => dispatch.div(ctx, (expr.left, expr.right)),
  };

  Some(result)
}

fn apply_post_eval<Ctx>(
  registry: &DispatchRegistry<Ctx>,
  ctx: &mut Ctx,
  result: i64,
) -> i64 {
  let Some(handler) = registry.get("post_eval") else {
    return result;
  };

  let output = handler(ctx, Box::new(result) as DynValue);
  match output.downcast::<i64>() {
    Ok(val) => *val,
    Err(_) => result,
  }
}

fn main() {
  let mut base = CalculatorDispatch::<CalcCtx, _, _, _, _, _>::new()
    .with_parse(|_ctx: &mut CalcCtx, input: String| parse_expr(&input))
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

  let post_eval_add_one: DynHandler<CalcCtx> = Arc::new(|ctx, input| {
    match input.downcast::<i64>() {
      Ok(val) => {
        ctx.note("post_eval: +1");
        Box::new(*val + 1) as DynValue
      }
      Err(input) => input,
    }
  });
  base.registry_mut().set("post_eval", post_eval_add_one);

  let mut tuned = base.clone();
  let post_eval_times_ten: DynHandler<CalcCtx> = Arc::new(|ctx, input| {
    match input.downcast::<i64>() {
      Ok(val) => {
        ctx.note("post_eval: *10");
        Box::new(*val * 10) as DynValue
      }
      Err(input) => input,
    }
  });
  tuned.registry_mut().set("post_eval", post_eval_times_ten);
  let tuned = tuned.with_add(|ctx: &mut CalcCtx, (a, b): (i64, i64)| {
    ctx.note("tuned add -> +100");
    a + b + 100
  });

  let inputs = ["1 + 2", "6 / 0", "3 * 4", "bad input"];
  let mut base_ctx = CalcCtx::default();
  for line in inputs {
    match eval_line(&base, &mut base_ctx, line) {
      Some(result) => {
        let result = apply_post_eval(base.registry(), &mut base_ctx, result);
        println!("base: {line} = {result}");
      }
      None => println!("base: {line} = error"),
    }
  }

  if !base_ctx.notes.is_empty() {
    println!("base notes: {:?}", base_ctx.notes);
  }

  let line = "2 + 2";
  let mut tuned_ctx = CalcCtx::default();
  match eval_line(&tuned, &mut tuned_ctx, line) {
    Some(result) => {
      let result = apply_post_eval(tuned.registry(), &mut tuned_ctx, result);
      println!("tuned: {line} = {result}");
    }
    None => println!("tuned: {line} = error"),
  }

  if !tuned_ctx.notes.is_empty() {
    println!("tuned notes: {:?}", tuned_ctx.notes);
  }
}
