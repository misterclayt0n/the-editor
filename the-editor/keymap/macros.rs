#[macro_export]
macro_rules! key {
  ($name:ident) => {{ $crate::keymap::binding_from_ident(stringify!($name)) }};
  ($lit:literal) => {{ $crate::keymap::binding_from_literal($lit) }};
}

#[macro_export]
macro_rules! keymap {
  ({ $name:literal $($rest:tt)* }) => {
    {
      use std::collections::HashMap;
      let mut _map: HashMap<$crate::keymap::KeyBinding, $crate::keymap::KeyTrie> = HashMap::new();
      let mut _order: Vec<$crate::keymap::KeyBinding> = Vec::new();
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
      if $map.insert(_k, $crate::keymap::KeyTrie::Command($crate::keymap::Command::Execute($crate::core::commands::$cmd))).is_none() { $order.push(_k); }
    )+
    $crate::keymap!(@pairs $map, $order; $($rest)*);
  };

  // single key => cmd,
  (@pairs $map:ident, $order:ident; $k:tt => $cmd:ident, $($rest:tt)*) => {
    let _k = $crate::key!($k);
    if $map.insert(_k, $crate::keymap::KeyTrie::Command($crate::keymap::Command::Execute($crate::core::commands::$cmd))).is_none() { $order.push(_k); }
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
