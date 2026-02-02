use the_lib::render::RenderPlan;

use crate::DefaultContext;

pub type RenderPass<Ctx> = Box<dyn Fn(&mut Ctx, &mut RenderPlan) + 'static>;

/// Render passes are run with a temporary take of the pass list.
/// Avoid mutating the render-pass registry from inside a pass.
pub fn run_render_passes<Ctx: DefaultContext>(ctx: &mut Ctx, plan: &mut RenderPlan) {
  let passes = std::mem::take(ctx.render_passes_mut());
  for pass in &passes {
    pass(ctx, plan);
  }
  *ctx.render_passes_mut() = passes;
}

pub fn default_render_passes<Ctx: DefaultContext>() -> Vec<RenderPass<Ctx>> {
  Vec::new()
}
