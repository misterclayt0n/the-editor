#[macro_export]
macro_rules! define {
  (
    $name:ident {
      $(
        $point:ident : $input:ty
      ),* $(,)?
    }
  ) => {
    $crate::paste::paste! {
      pub struct [<$name Dispatch>]<Ctx, $( [<$point:camel Handler>] ),* > {
        $(
          $point: [<$point:camel Handler>],
        )*
        #[cfg(feature = "dynamic-registry")]
        registry: $crate::DispatchRegistry<Ctx>,
        #[cfg(not(feature = "dynamic-registry"))]
        _ctx: ::std::marker::PhantomData<Ctx>,
      }

      impl<Ctx, $( [<$point:camel Handler>] ),* > [<$name Dispatch>]<Ctx, $( [<$point:camel Handler>] ),* >
      where
        $( [<$point:camel Handler>]: $crate::HandlerFn<Ctx, $input> ),*
      {
        $(
          pub fn $point(&self, ctx: &mut Ctx, input: $input) {
            self.$point.call(ctx, input)
          }
        )*

        #[cfg(feature = "dynamic-registry")]
        pub fn registry(&self) -> &$crate::DispatchRegistry<Ctx> {
          &self.registry
        }

        #[cfg(feature = "dynamic-registry")]
        pub fn registry_mut(&mut self) -> &mut $crate::DispatchRegistry<Ctx> {
          &mut self.registry
        }
      }

      impl<Ctx> [<$name Dispatch>]<Ctx, $( fn(&mut Ctx, $input) ),* > {
        pub fn new() -> Self {
          Self {
            $(
              $point: |_, _| {},
            )*
            #[cfg(feature = "dynamic-registry")]
            registry: $crate::DispatchRegistry::new(),
            #[cfg(not(feature = "dynamic-registry"))]
            _ctx: ::std::marker::PhantomData,
          }
        }
      }
    }

    $crate::__dispatch_builders!($name, $( $point : $input ),*);
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builders {
  ($name:ident, $( $point:ident : $input:ty ),* $(,)?) => {
    $crate::__dispatch_builders_inner!($name, (); $( $point : $input, )* );
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builders_inner {
  ($name:ident, ($($prefix:ident : $prefix_input:ty,)*); $point:ident : $input:ty, $( $rest:ident : $rest_input:ty, )* ) => {
    $crate::__dispatch_builder_for_point!(
      $name,
      $point : $input,
      ($($prefix : $prefix_input,)*),
      ( $( $rest : $rest_input, )* )
    );

    $crate::__dispatch_builders_inner!(
      $name,
      ($($prefix : $prefix_input,)* $point : $input,);
      $( $rest : $rest_input, )*
    );
  };
  ($name:ident, ($($prefix:ident : $prefix_input:ty,)*); ) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builder_for_point {
  (
    $name:ident,
    $point:ident : $input:ty,
    ($($prefix:ident : $prefix_input:ty,)*),
    ($($rest:ident : $rest_input:ty,)*)
  ) => {
    $crate::paste::paste! {
      impl<Ctx, $( [<$prefix:camel Handler>], )* [<$point:camel Handler>] $(, [<$rest:camel Handler>] )* >
        [<$name Dispatch>]<Ctx, $( [<$prefix:camel Handler>], )* [<$point:camel Handler>] $(, [<$rest:camel Handler>] )* >
      {
        pub fn [<with_ $point>]<NewHandler>(self, handler: NewHandler)
          -> [<$name Dispatch>]<Ctx, $( [<$prefix:camel Handler>], )* NewHandler $(, [<$rest:camel Handler>] )* >
        where
          NewHandler: $crate::HandlerFn<Ctx, $input>,
        {
          let Self {
            $( $prefix, )*
            $point: _,
            $( $rest, )*
            #[cfg(feature = "dynamic-registry")]
            registry,
            #[cfg(not(feature = "dynamic-registry"))]
            _ctx,
          } = self;

          [<$name Dispatch>] {
            $( $prefix: $prefix, )*
            $point: handler,
            $( $rest: $rest, )*
            #[cfg(feature = "dynamic-registry")]
            registry,
            #[cfg(not(feature = "dynamic-registry"))]
            _ctx,
          }
        }
      }
    }
  };
}
