#[macro_export]
macro_rules! key {
  // Named keys
  (Left) => { the_editor_renderer::Key::Left };
  (Right) => { the_editor_renderer::Key::Right };
  (Up) => { the_editor_renderer::Key::Up };
  (Down) => { the_editor_renderer::Key::Down };
  (Enter) => { the_editor_renderer::Key::Enter };
  (Esc) => { the_editor_renderer::Key::Escape };
  (Backspace) => { the_editor_renderer::Key::Backspace };
  (Space) => { the_editor_renderer::Key::Char(' ') };
  // Char keys
  ($ch:literal) => {{
    const C: char = $ch;
    the_editor_renderer::Key::Char(C)
  }};
}

#[macro_export]
macro_rules! keymap {
  ({ $name:literal $($rest:tt)* }) => {
    {
      use std::collections::HashMap;
      let mut _map: HashMap<the_editor_renderer::Key, $crate::keymap::KeyTrie> = HashMap::new();
      let mut _order: Vec<the_editor_renderer::Key> = Vec::new();
      $crate::keymap!(@pairs _map, _order; $($rest)*);
      $crate::keymap::KeyTrie::Node($crate::keymap::KeyTrieNode::new($name, _map, _order))
    }
  };

  // handle sticky attribute (ignored for now but parsed)
  (@pairs $map:ident, $order:ident; sticky=true $($rest:tt)*) => { keymap!(@pairs $map, $order; $($rest)* ); };

  // multiple key aliases: "h" | Left => cmd,
  (@pairs $map:ident, $order:ident; $($k:tt)|+ => $cmd:ident, $($rest:tt)*) => {
    $(
      let _k = $crate::key!($k);
      if $map.insert(_k, $crate::keymap::KeyTrie::Command($crate::keymap::Command::Execute(crate::core::commands::$cmd))).is_none() { $order.push(_k); }
    )+
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  // single key => cmd,
  (@pairs $map:ident, $order:ident; $k:tt => $cmd:ident, $($rest:tt)*) => {
    let _k = $crate::key!($k);
    if $map.insert(_k, $crate::keymap::KeyTrie::Command($crate::keymap::Command::Execute(crate::core::commands::$cmd))).is_none() { $order.push(_k); }
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  // nested: key => { "Name" ... },
  (@pairs $map:ident, $order:ident; $k:tt => { $name:literal $($inner:tt)* }, $($rest:tt)*) => {
    let _k = $crate::key!($k);
    let _node = $crate::keymap!({ $name $($inner)* });
    if $map.insert(_k, _node).is_none() { $order.push(_k); }
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  // done
  (@pairs $map:ident, $order:ident; ) => {};
}
