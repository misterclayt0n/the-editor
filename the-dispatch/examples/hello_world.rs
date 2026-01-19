use the_dispatch::define;

define! {
    Hello {
        greet: String => String,
    }
}

fn main() {
  let dispatch = HelloDispatch::<(), _>::new()
    .with_greet(|_ctx: &mut (), name: String| format!("hello, {name}"));

  let mut ctx = ();
  let message = dispatch.greet(&mut ctx, "world".to_string());

  println!("{message}");
}
