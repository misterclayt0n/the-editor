use std::cell::RefCell;
use std::rc::Rc;

use the_dispatch::define;

// Basic test context
struct TestCtx {
  log: Rc<RefCell<Vec<String>>>,
  value: i32,
}

impl TestCtx {
  fn new() -> Self {
    Self {
      log: Rc::new(RefCell::new(Vec::new())),
      value: 0,
    }
  }

  fn push(&self, msg: &str) {
    self.log.borrow_mut().push(msg.to_string());
  }

  fn logs(&self) -> Vec<String> {
    self.log.borrow().clone()
  }
}

// Define a simple dispatch for testing
define! {
    Test {
        on_event: i32,
        on_action: String,
    }
}

define! {
    OutputTest {
        double: i32 => i32,
        label: i32 => String,
    }
}

define! {
    ApiTest {
        inc: i32 => i32,
        tag: () => &'static str,
    }
}

#[test]
fn test_dispatch_new_creates_noop_handlers() {
  let dispatch = TestDispatch::<TestCtx, _, _>::new();
  let mut ctx = TestCtx::new();

  // Should not panic, just do nothing
  dispatch.on_event(&mut ctx, 42);
  dispatch.on_action(&mut ctx, "hello".to_string());

  assert!(ctx.logs().is_empty());
  assert_eq!(ctx.value, 0);
}

#[test]
fn test_dispatch_builder_replaces_handler() {
  let dispatch =
    TestDispatch::<TestCtx, _, _>::new().with_on_event(|ctx: &mut TestCtx, val: i32| {
      ctx.push(&format!("on_event: {}", val));
      ctx.value = val;
    });

  let mut ctx = TestCtx::new();
  dispatch.on_event(&mut ctx, 42);

  assert_eq!(ctx.logs(), vec!["on_event: 42"]);
  assert_eq!(ctx.value, 42);
}

#[test]
fn test_dispatch_builder_chain_replaces_multiple_handlers() {
  let dispatch = TestDispatch::<TestCtx, _, _>::new()
    .with_on_event(|ctx: &mut TestCtx, val: i32| {
      ctx.push(&format!("event: {}", val));
    })
    .with_on_action(|ctx: &mut TestCtx, action: String| {
      ctx.push(&format!("action: {}", action));
    });

  let mut ctx = TestCtx::new();
  dispatch.on_event(&mut ctx, 10);
  dispatch.on_action(&mut ctx, "test".to_string());

  assert_eq!(ctx.logs(), vec!["event: 10", "action: test"]);
}

#[test]
fn test_dispatch_builder_can_replace_handler_multiple_times() {
  let dispatch = TestDispatch::<TestCtx, _, _>::new()
    .with_on_event(|ctx: &mut TestCtx, _val: i32| {
      ctx.push("first handler");
    })
    .with_on_event(|ctx: &mut TestCtx, _val: i32| {
      ctx.push("second handler");
    });

  let mut ctx = TestCtx::new();
  dispatch.on_event(&mut ctx, 0);

  // Only the second handler should be called
  assert_eq!(ctx.logs(), vec!["second handler"]);
}

#[test]
fn test_dispatch_returns_handler_outputs() {
  let dispatch = OutputTestDispatch::<(), _, _>::new()
    .with_double(|_ctx: &mut (), val: i32| val * 2)
    .with_label(|_ctx: &mut (), val: i32| format!("val: {}", val));

  let mut ctx = ();
  let doubled = dispatch.double(&mut ctx, 21);
  let label = dispatch.label(&mut ctx, 7);

  assert_eq!(doubled, 42);
  assert_eq!(label, "val: 7");
}

#[test]
fn test_default_handlers_return_default_output() {
  let dispatch = OutputTestDispatch::<(), _, _>::new();
  let mut ctx = ();

  assert_eq!(dispatch.double(&mut ctx, 9), 0);
  assert_eq!(dispatch.label(&mut ctx, 9), String::new());
}

fn call_api<Ctx>(dispatch: &impl ApiTestApi<Ctx>, ctx: &mut Ctx) -> (i32, &'static str) {
  (dispatch.inc(ctx, 1), dispatch.tag(ctx, ()))
}

#[test]
fn test_dispatch_api_trait_is_ergonomic() {
  let dispatch = ApiTestDispatch::<(), _, _>::new()
    .with_inc(|_ctx: &mut (), val: i32| val + 1)
    .with_tag(|_ctx: &mut (), _: ()| "ok");

  let mut ctx = ();
  assert_eq!(call_api(&dispatch, &mut ctx), (2, "ok"));
}

// Define a more complex dispatch for testing inter-handler calls
define! {
    Chain {
        pre_process: i32,
        process: i32,
        post_process: i32,
    }
}

#[test]
fn test_handlers_can_simulate_chain_via_external_coordination() {
  // This test demonstrates the dispatch chain pattern:
  // Handlers don't call each other directly; instead, control flow
  // is managed externally or via callbacks.

  let log = Rc::new(RefCell::new(Vec::new()));

  let dispatch = ChainDispatch::<(), _, _, _>::new()
    .with_pre_process({
      let log = log.clone();
      move |_ctx: &mut (), val: i32| {
        log.borrow_mut().push(format!("pre: {}", val));
      }
    })
    .with_process({
      let log = log.clone();
      move |_ctx: &mut (), val: i32| {
        log.borrow_mut().push(format!("process: {}", val));
      }
    })
    .with_post_process({
      let log = log.clone();
      move |_ctx: &mut (), val: i32| {
        log.borrow_mut().push(format!("post: {}", val));
      }
    });

  let mut ctx = ();

  // The chain is invoked explicitly by the caller
  let val = 5;
  dispatch.pre_process(&mut ctx, val);
  let val2 = val * 2;
  dispatch.process(&mut ctx, val2);
  let val3 = val2 + 1;
  dispatch.post_process(&mut ctx, val3);

  let logs: Vec<String> = log.borrow().clone();
  assert_eq!(logs, vec!["pre: 5", "process: 10", "post: 11"]);
}

// Test with closures that capture state
#[test]
fn test_handlers_can_be_closures_with_captured_state() {
  let counter = Rc::new(RefCell::new(0));
  let counter_clone = counter.clone();

  let dispatch =
    TestDispatch::<TestCtx, _, _>::new().with_on_event(move |_ctx: &mut TestCtx, val: i32| {
      *counter_clone.borrow_mut() += val;
    });

  let mut ctx = TestCtx::new();
  dispatch.on_event(&mut ctx, 10);
  dispatch.on_event(&mut ctx, 20);
  dispatch.on_event(&mut ctx, 30);

  assert_eq!(*counter.borrow(), 60);
}

// Define dispatch with unit type
define! {
    Signal {
        trigger: (),
        notify: (),
    }
}

#[test]
fn test_dispatch_with_unit_input() {
  let dispatch = SignalDispatch::<TestCtx, _, _>::new()
    .with_trigger(|ctx: &mut TestCtx, _: ()| {
      ctx.push("triggered");
    })
    .with_notify(|ctx: &mut TestCtx, _: ()| {
      ctx.push("notified");
    });

  let mut ctx = TestCtx::new();
  dispatch.trigger(&mut ctx, ());
  dispatch.notify(&mut ctx, ());

  assert_eq!(ctx.logs(), vec!["triggered", "notified"]);
}

// Define dispatch with complex types
#[derive(Clone, Debug, PartialEq)]
struct KeyEvent {
  key: char,
  modifiers: u8,
}

#[derive(Clone, Debug, PartialEq)]
enum Action {
  Insert(char),
  Delete,
  Move(i32, i32),
}

define! {
    Editor {
        pre_on_keypress: KeyEvent,
        on_keypress: KeyEvent,
        post_on_keypress: Action,
        on_action: Action,
    }
}

#[test]
fn test_dispatch_with_complex_types() {
  let dispatch = EditorDispatch::<TestCtx, _, _, _, _>::new()
    .with_on_keypress(|ctx: &mut TestCtx, key: KeyEvent| {
      ctx.push(&format!("key: {} mod: {}", key.key, key.modifiers));
    })
    .with_on_action(|ctx: &mut TestCtx, action: Action| {
      ctx.push(&format!("action: {:?}", action));
    });

  let mut ctx = TestCtx::new();
  dispatch.on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'a',
      modifiers: 0,
    },
  );
  dispatch.on_action(&mut ctx, Action::Insert('b'));

  assert_eq!(ctx.logs(), vec!["key: a mod: 0", "action: Insert('b')"]);
}

// Test that dispatch is zero-cost (compiles to direct function calls)
// This is a compile-time test - if it compiles, the generics work correctly
#[test]
fn test_dispatch_is_generic_over_handler_types() {
  // Using closures (different concrete types per closure)
  let dispatch1 = TestDispatch::<TestCtx, _, _>::new()
    .with_on_event(|_ctx: &mut TestCtx, _val: i32| {})
    .with_on_action(|_ctx: &mut TestCtx, _action: String| {});

  // Using a different set of closures
  let dispatch2 = TestDispatch::<TestCtx, _, _>::new()
    .with_on_event(|ctx: &mut TestCtx, val: i32| ctx.value = val)
    .with_on_action(|ctx: &mut TestCtx, action: String| ctx.push(&action));

  // Both dispatches compile with different handler types
  let mut ctx = TestCtx::new();
  dispatch1.on_event(&mut ctx, 1);
  dispatch2.on_event(&mut ctx, 2);
  assert_eq!(ctx.value, 2);
}

// Test Default impl for handlers
#[test]
fn test_default_handlers_are_noops() {
  let dispatch = EditorDispatch::<TestCtx, _, _, _, _>::new();
  let mut ctx = TestCtx::new();

  // All default handlers should be no-ops
  dispatch.pre_on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'x',
      modifiers: 0,
    },
  );
  dispatch.on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'x',
      modifiers: 0,
    },
  );
  dispatch.post_on_keypress(&mut ctx, Action::Delete);
  dispatch.on_action(&mut ctx, Action::Move(1, 2));

  assert!(ctx.logs().is_empty());
}

// Test that handlers receive correct inputs
#[test]
fn test_handlers_receive_correct_inputs() {
  let received = Rc::new(RefCell::new(Vec::new()));
  let received_clone = received.clone();

  let dispatch = EditorDispatch::<(), _, _, _, _>::new().with_on_keypress(
    move |_ctx: &mut (), key: KeyEvent| {
      received_clone.borrow_mut().push(format!("key:{}", key.key));
    },
  );

  let mut ctx = ();
  dispatch.on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'a',
      modifiers: 0,
    },
  );
  dispatch.on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'b',
      modifiers: 1,
    },
  );
  dispatch.on_keypress(
    &mut ctx,
    KeyEvent {
      key: 'c',
      modifiers: 2,
    },
  );

  assert_eq!(*received.borrow(), vec!["key:a", "key:b", "key:c"]);
}
