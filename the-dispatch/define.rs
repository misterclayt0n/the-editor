#[macro_export]
macro_rules! define {
  (
    $name:ident {
      $(
        $point:ident : $input:ty $(=> $output:ty)?
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
        $( [<$point:camel Handler>]:
          $crate::HandlerFn<Ctx, $input, $crate::__dispatch_output!($($output)?)>
        ),*
      {
        $(
          pub fn $point(
            &self,
            ctx: &mut Ctx,
            input: $input
          ) -> $crate::__dispatch_output!($($output)?) {
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

      impl<Ctx> [<$name Dispatch>]<Ctx, $( fn(&mut Ctx, $input) -> $crate::__dispatch_output!($($output)?) ),* >
      where
        $( $crate::__dispatch_output!($($output)?): ::std::default::Default ),*
      {
        pub fn new() -> Self {
          Self {
            $(
              $point: |_, _| ::std::default::Default::default(),
            )*
            #[cfg(feature = "dynamic-registry")]
            registry: $crate::DispatchRegistry::new(),
            #[cfg(not(feature = "dynamic-registry"))]
            _ctx: ::std::marker::PhantomData,
          }
        }
      }

      pub trait [<$name Api>]<Ctx> {
        $(
          fn $point(
            &self,
            ctx: &mut Ctx,
            input: $input
          ) -> $crate::__dispatch_output!($($output)?);
        )*
      }

      impl<Ctx, $( [<$point:camel Handler>] ),* > [<$name Api>]<Ctx>
        for [<$name Dispatch>]<Ctx, $( [<$point:camel Handler>] ),* >
      where
        $( [<$point:camel Handler>]:
          $crate::HandlerFn<Ctx, $input, $crate::__dispatch_output!($($output)?)>
        ),*
      {
        $(
          fn $point(
            &self,
            ctx: &mut Ctx,
            input: $input
          ) -> $crate::__dispatch_output!($($output)?) {
            self.$point.call(ctx, input)
          }
        )*
      }
    }

    $crate::__dispatch_builders!($name, $( $point : $input $(=> $output)? ),*);
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_output {
  () => { () };
  ($output:ty) => { $output };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builders {
  ($name:ident, $( $point:ident : $input:ty $(=> $output:ty)? ),* $(,)?) => {
    $crate::__dispatch_builders_inner!($name, (); $( $point : $input $(=> $output)?, )* );
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builders_inner {
  (
    $name:ident,
    ($($prefix:ident : $prefix_input:ty $(=> $prefix_output:ty)?,)*);
    $point:ident : $input:ty $(=> $output:ty)?,
    $( $rest:ident : $rest_input:ty $(=> $rest_output:ty)?, )*
  ) => {
    $crate::__dispatch_builder_for_point!(
      $name,
      $point : $input $(=> $output)?,
      ($($prefix : $prefix_input $(=> $prefix_output)?,)*),
      ( $( $rest : $rest_input $(=> $rest_output)?, )* )
    );

    $crate::__dispatch_builders_inner!(
      $name,
      ($($prefix : $prefix_input $(=> $prefix_output)?,)* $point : $input $(=> $output)?,);
      $( $rest : $rest_input $(=> $rest_output)?, )*
    );
  };
  ($name:ident, ($($prefix:ident : $prefix_input:ty $(=> $prefix_output:ty)?,)*); ) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_builder_for_point {
  (
    $name:ident,
    $point:ident : $input:ty $(=> $output:ty)?,
    ($($prefix:ident : $prefix_input:ty $(=> $prefix_output:ty)?,)*),
    ($($rest:ident : $rest_input:ty $(=> $rest_output:ty)?,)*)
  ) => {
    $crate::paste::paste! {
      impl<Ctx, $( [<$prefix:camel Handler>], )* [<$point:camel Handler>] $(, [<$rest:camel Handler>] )* >
        [<$name Dispatch>]<Ctx, $( [<$prefix:camel Handler>], )* [<$point:camel Handler>] $(, [<$rest:camel Handler>] )* >
      {
        pub fn [<with_ $point>]<NewHandler>(self, handler: NewHandler)
          -> [<$name Dispatch>]<Ctx, $( [<$prefix:camel Handler>], )* NewHandler $(, [<$rest:camel Handler>] )* >
        where
          NewHandler: $crate::HandlerFn<Ctx, $input, $crate::__dispatch_output!($($output)?)>,
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
