#[cfg(feature = "dynamic-registry")] use std::sync::Arc;

use the_dispatch::define;
#[cfg(feature = "dynamic-registry")]
use the_dispatch::{
  DynHandler,
  DynValue,
};

#[derive(Clone, Copy, Debug)]
enum Op {
  Add,
  Sub,
  Mul,
  Div,
}

#[derive(Clone, Copy, Debug)]
struct Expr {
  left:  i64,
  op:    Op,
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

type CalcDispatch = CalculatorDispatch<
  CalcCtx,
  fn(&mut CalcCtx, String) -> Option<Expr>,
  fn(&mut CalcCtx, (i64, i64)) -> i64,
  fn(&mut CalcCtx, (i64, i64)) -> i64,
  fn(&mut CalcCtx, (i64, i64)) -> i64,
  fn(&mut CalcCtx, (i64, i64)) -> i64,
>;

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

fn parse_handler(_ctx: &mut CalcCtx, input: String) -> Option<Expr> {
  parse_expr(&input)
}

fn add_handler(_ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
  a + b
}

fn sub_handler(_ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
  a - b
}

fn mul_handler(_ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
  a * b
}

fn div_handler(ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
  if b == 0 {
    ctx.note("divide by zero -> returning 0");
    0
  } else {
    a / b
  }
}

fn tuned_add_handler(ctx: &mut CalcCtx, (a, b): (i64, i64)) -> i64 {
  ctx.note("tuned add -> +100");
  a + b + 100
}

fn build_dispatch() -> CalcDispatch {
  CalculatorDispatch::new()
    .with_parse(parse_handler as fn(&mut CalcCtx, String) -> Option<Expr>)
    .with_add(add_handler as fn(&mut CalcCtx, (i64, i64)) -> i64)
    .with_sub(sub_handler as fn(&mut CalcCtx, (i64, i64)) -> i64)
    .with_mul(mul_handler as fn(&mut CalcCtx, (i64, i64)) -> i64)
    .with_div(div_handler as fn(&mut CalcCtx, (i64, i64)) -> i64)
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

#[cfg(feature = "dynamic-registry")]
fn install_post_eval_add_one(dispatch: &mut CalcDispatch) {
  let handler: DynHandler<CalcCtx> = Arc::new(|ctx, input| {
    match input.downcast::<i64>() {
      Ok(val) => {
        ctx.note("post_eval: +1");
        Box::new(*val + 1) as DynValue
      },
      Err(input) => input,
    }
  });
  dispatch.registry_mut().set("post_eval", handler);
}

#[cfg(not(feature = "dynamic-registry"))]
fn install_post_eval_add_one(_dispatch: &mut CalcDispatch) {}

#[cfg(feature = "dynamic-registry")]
fn install_post_eval_times_ten(dispatch: &mut CalcDispatch) {
  let handler: DynHandler<CalcCtx> = Arc::new(|ctx, input| {
    match input.downcast::<i64>() {
      Ok(val) => {
        ctx.note("post_eval: *10");
        Box::new(*val * 10) as DynValue
      },
      Err(input) => input,
    }
  });
  dispatch.registry_mut().set("post_eval", handler);
}

#[cfg(not(feature = "dynamic-registry"))]
fn install_post_eval_times_ten(_dispatch: &mut CalcDispatch) {}

#[cfg(feature = "dynamic-registry")]
fn apply_post_eval(dispatch: &CalcDispatch, ctx: &mut CalcCtx, result: i64) -> i64 {
  let Some(handler) = dispatch.registry().get("post_eval") else {
    return result;
  };

  let output = handler(ctx, Box::new(result) as DynValue);
  match output.downcast::<i64>() {
    Ok(val) => *val,
    Err(_) => result,
  }
}

#[cfg(not(feature = "dynamic-registry"))]
fn apply_post_eval(_dispatch: &CalcDispatch, _ctx: &mut CalcCtx, result: i64) -> i64 {
  result
}

fn main() {
  let mut base = build_dispatch();
  install_post_eval_add_one(&mut base);

  #[cfg(feature = "cow-handlers")]
  let mut tuned = base.clone();

  #[cfg(not(feature = "cow-handlers"))]
  let mut tuned = build_dispatch();

  install_post_eval_times_ten(&mut tuned);
  let tuned = tuned.with_add(tuned_add_handler as fn(&mut CalcCtx, (i64, i64)) -> i64);

  let inputs = ["1 + 2", "6 / 0", "3 * 4", "bad input"];
  let mut base_ctx = CalcCtx::default();

  for line in inputs {
    let mut result = eval_line(&base, &mut base_ctx, line);
    if let Some(value) = result {
      result = Some(apply_post_eval(&base, &mut base_ctx, value));
    }

    match result {
      Some(value) => println!("base: {line} = {value}"),
      None => println!("base: {line} = error"),
    }
  }

  if !base_ctx.notes.is_empty() {
    println!("base notes: {:?}", base_ctx.notes);
  }

  let line = "2 + 2";
  let mut tuned_ctx = CalcCtx::default();
  let mut result = eval_line(&tuned, &mut tuned_ctx, line);
  if let Some(value) = result {
    result = Some(apply_post_eval(&tuned, &mut tuned_ctx, value));
  }

  match result {
    Some(value) => println!("tuned: {line} = {value}"),
    None => println!("tuned: {line} = error"),
  }

  if !tuned_ctx.notes.is_empty() {
    println!("tuned notes: {:?}", tuned_ctx.notes);
  }
}
