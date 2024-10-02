use skia_bindings as sb;

pub type Align = sb::SkSVGPreserveAspectRatio_Align;
pub type Scale = sb::SkSVGPreserveAspectRatio_Scale;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PreserveAspectRatio {
    pub align: Align,
    pub scale: Scale,
}

impl PreserveAspectRatio {
    pub fn new(align: Align, scale: Scale) -> Self {
        Self { align, scale }
    }
}

native_transmutable!(
    sb::SkSVGPreserveAspectRatio,
    PreserveAspectRatio,
    svg_preserve_aspect_ratio_layout
);
