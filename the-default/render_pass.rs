use the_lib::render::RenderPlan;

use crate::{
  DefaultContext,
  command_palette::build_command_palette_overlay_with_style,
};

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

fn command_palette_pass<Ctx: DefaultContext>(ctx: &mut Ctx, plan: &mut RenderPlan) {
  let overlays =
    build_command_palette_overlay_with_style(ctx.command_palette(), plan.viewport, ctx.command_palette_style());
  if !overlays.is_empty() {
    plan.overlays.extend(overlays);
  }
}

pub fn default_render_passes<Ctx: DefaultContext>() -> Vec<RenderPass<Ctx>> {
  Vec::new()
}

/// Optional helper render pass for clients that want command palette overlays
/// built from the default layout helpers (e.g. the-term).
pub fn command_palette_overlay_pass<Ctx: DefaultContext>() -> RenderPass<Ctx> {
  Box::new(command_palette_pass::<Ctx>)
}
