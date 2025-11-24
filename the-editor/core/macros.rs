//! These are macros to make getting very nested fields in the `Editor` struct
//! easier These are macros instead of functions because functions will have to
//! take `&mut self` However, rust doesn't know that you only want a partial
//! borrow instead of borrowing the entire struct which `&mut self` says.  This
//! makes it impossible to do other mutable stuff to the struct because it is
//! already borrowed. Because macros are expanded, this circumvents the problem
//! because it is just like indexing fields by hand and then putting a `&mut` in
//! front of it. This way rust can see that we are only borrowing a part of the
//! struct and not the entire thing.

/// Get the current view and document mutably as a tuple.
/// Returns `(&mut View, &mut Document)`
///
/// # Panics
/// Panics if the focused node is not a view.
#[macro_export]
macro_rules! current {
  ($editor:expr) => {{
    let view = $crate::view_mut!($editor);
    let id = view.doc;
    let doc = $crate::doc_mut!($editor, &id);
    (view, doc)
  }};
}

#[macro_export]
macro_rules! current_ref {
  ($editor:expr) => {{
    let view_id = $editor
      .focused_view_id()
      .expect("no active document view available");
    let view = $editor.tree.get(view_id);
    let doc = &$editor.documents[&view.doc];
    (view, doc)
  }};
}

/// Get the current document mutably.
/// Returns `&mut Document`
#[macro_export]
macro_rules! doc_mut {
  ($editor:expr, $id:expr) => {{ $editor.documents.get_mut($id).unwrap() }};
  ($editor:expr) => {{ $crate::current!($editor).1 }};
}

/// Get the current view mutably.
/// Returns `&mut View`
///
/// # Panics
/// Panics if the ID doesn't exist.
#[macro_export]
macro_rules! view_mut {
  ($editor:expr, $id:expr) => {{ $editor.tree.get_mut($id) }};
  ($editor:expr) => {{
    // let view_id = $editor
    // .focused_view_id()
    // .expect("no active document view available");
    $editor.tree.get_mut($editor.tree.focus)
  }};
}

/// Get the current view immutably
/// Returns `&View`
///
/// # Panics
/// Panics if the ID doesn't exist.
#[macro_export]
macro_rules! view {
  ($editor:expr, $id:expr) => {{ $editor.tree.get($id) }};
  ($editor:expr) => {{
    let view_id = $editor
      .focused_view_id()
      .expect("no active document view available");
    $editor.tree.get(view_id)
  }};
}

/// Check if the focused node is a view
#[macro_export]
macro_rules! focus_is_view {
  ($editor:expr) => {{ $editor.tree.try_get($editor.tree.focus).is_some() }};
}

#[macro_export]
macro_rules! doc {
  ($editor:expr, $id:expr) => {{ &$editor.documents[$id] }};
  ($editor:expr) => {{ $crate::current_ref!($editor).1 }};
}

#[macro_export]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = hashmap!(@count $($key),*);
            let mut _map = ::std::collections::HashMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
    };
}
