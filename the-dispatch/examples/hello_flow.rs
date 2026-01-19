use the_dispatch::define;

#[derive(Default)]
struct Ctx {
  log: Vec<String>,
}

impl Ctx {
  fn note(&mut self, note: &str) {
    self.log.push(note.to_string())
  }
}

define! {
    HelloFlow {
        pre_greet: String => String,
        on_greet: String => String,
        post_greet: String => String,
    }
}

fn main() {
  let dispatch = HelloFlowDispatch::<Ctx, _, _, _>::new()
    .with_pre_greet(|ctx: &mut Ctx, name: String| {
      ctx.note("pre greet");
      name.trim().to_string()
    })
    .with_on_greet(|ctx: &mut Ctx, name: String| {
      ctx.note("on greet");
      format!("hello, {name}")
    })
    .with_post_greet(|ctx: &mut Ctx, greeting: String| {
      ctx.note("post greet");
      greeting
    });

  let mut ctx = Ctx::default();
  let name = "  world  ".to_string();

  let name = dispatch.pre_greet(&mut ctx, name);
  let greeting = dispatch.on_greet(&mut ctx, name);
  let greeting = dispatch.post_greet(&mut ctx, greeting);

  println!("{greeting}");
  println!("log: {:?}", ctx.log);
}
