use crate::{
    prelude::*, ColorType, Data, ImageInfo, Pixmap, YUVAIndex, YUVAInfo, YUVASizeInfo,
    YUVColorSpace,
};
use skia_bindings as sb;
use skia_bindings::{SkYUVAPixmapInfo, SkYUVAPixmaps};
use std::{ffi::c_void, ptr, slice};
use yuva_pixmap_info::{DataType, SupportedDataTypes};

/// [YUVAInfo] combined with per-plane [ColorType]s and row bytes. Fully specifies the [Pixmap]`s
/// for a YUVA image without the actual pixel memory and data.
pub type YUVAPixmapInfo = Handle<SkYUVAPixmapInfo>;

impl NativeDrop for SkYUVAPixmapInfo {
    fn drop(&mut self) {
        unsafe { sb::C_SkYUVAPixmapInfo_destruct(self) }
    }
}

impl NativePartialEq for SkYUVAPixmapInfo {
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { sb::C_SkYUVAPixmapInfo_equals(self, rhs) }
    }
}

impl YUVAPixmapInfo {
    pub const MAX_PLANES: usize = sb::SkYUVAInfo_kMaxPlanes as _;
    pub const DATA_TYPE_CNT: usize = DataType::Last as _;

    /// Initializes the [YUVAPixmapInfo] from a [YUVAInfo] with per-plane color types and row bytes.
    /// This will return [None] if the colorTypes aren't compatible with the [YUVAInfo] or if a
    /// rowBytes entry is not valid for the plane dimensions and color type. Color type and
    /// row byte values beyond the number of planes in [YUVAInfo] are ignored. All [ColorType]s
    /// must have the same [DataType] or this will return [None].
    ///
    /// If `rowBytes` is [None] then bpp*width is assumed for each plane.
    pub fn new(
        info: &YUVAInfo,
        color_types: &[ColorType; Self::MAX_PLANES],
        row_bytes: Option<&[usize; Self::MAX_PLANES]>,
    ) -> Option<Self> {
        let info = unsafe {
            SkYUVAPixmapInfo::new(
                info.native(),
                color_types.native().as_ptr(),
                row_bytes.map(|rb| rb.as_ptr()).unwrap_or(ptr::null()),
            )
        };
        Self::native_is_valid(&info).if_true_then_some(|| Self::from_native_c(info))
    }

    /// Like above but uses [yuva_pixmap_info::default_color_type_for_data_type] to determine each plane's [ColorType]. If
    /// `rowBytes` is [None] then bpp*width is assumed for each plane.
    pub fn from_data_type(
        info: &YUVAInfo,
        data_type: DataType,
        row_bytes: Option<&[usize; Self::MAX_PLANES]>,
    ) -> Option<Self> {
        let info = unsafe {
            SkYUVAPixmapInfo::new1(
                info.native(),
                data_type,
                row_bytes.map(|rb| rb.as_ptr()).unwrap_or(ptr::null()),
            )
        };
        Self::native_is_valid(&info).if_true_then_some(|| Self::from_native_c(info))
    }

    pub fn yuva_info(&self) -> &YUVAInfo {
        YUVAInfo::from_native_ref(&self.native().fYUVAInfo)
    }

    pub fn yuv_color_space(&self) -> YUVColorSpace {
        self.yuva_info().yuv_color_space()
    }

    /// The number of [Pixmap] planes.
    pub fn num_planes(&self) -> usize {
        self.yuva_info().num_planes()
    }

    /// The per-YUV`[A]` channel data type.
    pub fn data_type(&self) -> DataType {
        self.native().fDataType
    }

    /// Row bytes for the ith plane. Returns [None] if `i` >= [numPlanes(&self)] or this [YUVAPixmapInfo] is
    /// invalid.
    pub fn row_bytes(&self, i: usize) -> Option<usize> {
        (i < self.num_planes()).if_true_then_some(|| unsafe {
            sb::C_SkYUVAPixmapInfo_rowBytes(self.native(), i.try_into().unwrap())
        })
    }

    /// Image info for the ith plane, or [None] if `i` >= [numPlanes(&self)]
    pub fn plane_info(&self, i: usize) -> Option<&ImageInfo> {
        (i < self.num_planes()).if_true_then_some(|| {
            ImageInfo::from_native_ref(unsafe {
                &*sb::C_SkYUVAPixmapInfo_planeInfo(self.native(), i.try_into().unwrap())
            })
        })
    }

    /// Determine size to allocate for all planes. Optionally retrieves the per-plane sizes in
    /// planeSizes if not [None]. If total size overflows will return SIZE_MAX and set all `plane_sizes`
    /// to SIZE_MAX.
    pub fn compute_total_bytes(
        &self,
        plane_sizes: Option<&mut [usize; Self::MAX_PLANES]>,
    ) -> usize {
        unsafe {
            self.native().computeTotalBytes(
                plane_sizes
                    .map(|ps| ps.as_mut_ptr())
                    .unwrap_or(ptr::null_mut()),
            )
        }
    }

    /// Takes an allocation that is assumed to be at least [compute_total_bytes(&self)] in size and configures
    /// the first [numPlanes(&self)] entries in pixmaps array to point into that memory. The remaining
    /// entries of pixmaps are default initialized. Returns [None] if this [YUVAPixmapInfo] not valid.
    #[allow(clippy::clippy::missing_safety_doc)]
    pub unsafe fn init_pixmaps_from_single_allocation(
        &self,
        memory: *mut c_void,
    ) -> Option<[Pixmap; Self::MAX_PLANES]> {
        let mut pixmaps: [Pixmap; Self::MAX_PLANES] = Default::default();
        self.native()
            .initPixmapsFromSingleAllocation(memory, pixmaps.native_mut().as_mut_ptr())
            .if_true_some(pixmaps)
    }

    /// Returns `true` if this has been configured with a non-empty dimensioned [YUVAInfo] with
    /// compatible color types and row bytes.
    fn native_is_valid(info: *const SkYUVAPixmapInfo) -> bool {
        unsafe { sb::C_SkYUVAPixmapInfo_isValid(info) }
    }

    /// Is this valid and does it use color types allowed by the passed [SupportedDataTypes]?
    pub fn is_supported(&self, data_types: &SupportedDataTypes) -> bool {
        unsafe { self.native().isSupported(data_types.native()) }
    }
}

/// Helper to store [Pixmap] planes as described by a [YUVAPixmapInfo]. Can be responsible for
/// allocating/freeing memory for pixmaps or use external memory.
pub type YUVAPixmaps = Handle<SkYUVAPixmaps>;

impl NativeDrop for SkYUVAPixmaps {
    fn drop(&mut self) {
        unsafe { sb::C_SkYUVAPixmaps_destruct(self) }
    }
}

impl YUVAPixmaps {
    pub const MAX_PLANES: usize = YUVAPixmapInfo::MAX_PLANES;

    /// Allocate space for pixmaps' pixels in the [YUVAPixmaps].
    pub fn allocate(info: &YUVAPixmapInfo) -> Option<Self> {
        Self::try_construct(|pixmaps| unsafe {
            sb::C_SkYUVAPixmaps_Allocate(pixmaps, info.native());
            Self::native_is_valid(pixmaps)
        })
    }

    /// Use storage in [Data] as backing store for pixmaps' pixels. [Data] is retained by the
    /// [YUVAPixmaps].
    pub fn from_data(info: &YUVAPixmapInfo, data: impl Into<Data>) -> Option<Self> {
        Self::try_construct(|pixmaps| unsafe {
            sb::C_SkYUVAPixmaps_FromData(pixmaps, info.native(), data.into().into_ptr());
            Self::native_is_valid(pixmaps)
        })
    }

    /// Use passed in memory as backing store for pixmaps' pixels. Caller must ensure memory remains
    /// allocated while pixmaps are in use. There must be at least
    /// [YUVAPixmapInfo::computeTotalBytes(&self)] allocated starting at memory.
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_external_memory(info: &YUVAPixmapInfo, memory: *mut c_void) -> Option<Self> {
        Self::try_construct(|pixmaps| {
            sb::C_SkYUVAPixmaps_FromExternalMemory(pixmaps, info.native(), memory);
            Self::native_is_valid(pixmaps)
        })
    }

    /// Wraps existing `Pixmap`s. The [YUVAPixmaps] will have no ownership of the [Pixmap]s' pixel
    /// memory so the caller must ensure it remains valid. Will return [None] if
    /// the [YUVAInfo] isn't compatible with the [Pixmap] array (number of planes, plane dimensions,
    /// sufficient color channels in planes, ...).
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_external_pixmaps(
        info: &YUVAInfo,
        x_pixmaps: &[Pixmap; Self::MAX_PLANES],
    ) -> Option<Self> {
        Self::try_construct(|pixmaps| {
            sb::C_SkYUVAPixmaps_FromExternalPixmaps(
                pixmaps,
                info.native(),
                x_pixmaps.native().as_ptr(),
            );
            Self::native_is_valid(pixmaps)
        })
    }

    pub fn yuva_info(&self) -> &YUVAInfo {
        YUVAInfo::from_native_ref(&self.native().fYUVAInfo)
    }

    /// Number of pixmap planes.
    pub fn num_planes(&self) -> usize {
        self.yuva_info().num_planes()
    }

    /// Access the [Pixmap] planes.
    pub fn planes(&self) -> &[Pixmap] {
        unsafe {
            let planes = Pixmap::from_native_ref(&*sb::C_SkYUVAPixmaps_planes(self.native()));
            slice::from_raw_parts(planes, self.num_planes())
        }
    }

    /// Get the ith [Pixmap] plane. `Pixmap` will be default initialized if i >= numPlanes.
    pub fn plane(&self, i: usize) -> &Pixmap {
        &self.planes()[i]
    }

    /// Conversion to legacy YUVA data structures.
    pub fn to_legacy(&self) -> Option<(YUVASizeInfo, [YUVAIndex; 4])> {
        let mut info = YUVASizeInfo::default();
        let mut index = [YUVAIndex::default(); 4];
        unsafe {
            self.native()
                .toLegacy(info.native_mut(), &mut index.native_mut()[0])
        }
        .if_true_some((info, index))
    }

    fn native_is_valid(pixmaps: *const SkYUVAPixmaps) -> bool {
        unsafe { sb::C_SkYUVAPixmaps_isValid(pixmaps) }
    }
}

pub mod yuva_pixmap_info {
    use crate::{prelude::*, ColorType};
    use skia_bindings as sb;
    use skia_bindings::SkYUVAPixmapInfo_SupportedDataTypes;

    pub use crate::yuva_info::PlanarConfig;

    /// Data type for Y, U, V, and possibly A channels independent of how values are packed into
    /// planes.
    pub use skia_bindings::SkYUVAPixmapInfo_DataType as DataType;

    pub type SupportedDataTypes = Handle<SkYUVAPixmapInfo_SupportedDataTypes>;

    impl NativeDrop for SkYUVAPixmapInfo_SupportedDataTypes {
        fn drop(&mut self) {
            unsafe { sb::C_SkYUVAPixmapInfo_SupportedDataTypes_destruct(self) }
        }
    }

    impl Default for SupportedDataTypes {
        /// Defaults to nothing supported.
        fn default() -> Self {
            Self::construct(|sdt| unsafe {
                sb::C_SkYUVAPixmapInfo_SupportedDataTypes_Construct(sdt)
            })
        }
    }

    impl SupportedDataTypes {
        /// Init based on texture formats supported by the context.
        #[cfg(feature = "gpu")]
        pub fn from_context(context: &crate::gpu::RecordingContext) -> Self {
            Handle::from_native_c(unsafe {
                sb::SkYUVAPixmapInfo_SupportedDataTypes::new(
                    context.native() as *const _ as *const sb::GrImageContext
                )
            })
        }

        /// All legal combinations of [PlanarConfig] and [DataType] are supported.
        pub fn all() -> Self {
            Handle::construct(|sdt| unsafe { sb::C_SkYUVAPixmapInfo_SupportedDataTypes_All(sdt) })
        }

        /// Checks whether there is a supported combination of color types for planes structured
        /// as indicated by [PlanarConfig] with channel data types as indicated by [DataType].
        pub fn supported(&self, pc: PlanarConfig, dt: DataType) -> bool {
            unsafe { sb::C_SkYUVAPixmapInfo_SupportedDataTypes_supported(self.native(), pc, dt) }
        }

        /// Update to add support for pixmaps with `num_channels` channels where each channel is
        /// represented as [DataType].
        pub fn enable_data_type(&mut self, dt: DataType, num_channels: usize) {
            unsafe {
                self.native_mut()
                    .enableDataType(dt, num_channels.try_into().unwrap())
            }
        }
    }

    /// Gets the default [ColorType] to use with `num_channels` channels, each represented as [DataType].
    /// Returns [ColorType::Unknown] if no such color type.
    pub fn default_color_type_for_data_type(dt: DataType, num_channels: usize) -> ColorType {
        ColorType::from_native_c(unsafe {
            sb::C_SkYUVAPixmapInfo_DefaultColorTypeForDataType(dt, num_channels.try_into().unwrap())
        })
    }

    /// If the [ColorType] is supported for YUVA pixmaps this will return the number of YUVA channels
    /// that can be stored in a plane of this color type and what the [DataType] is of those channels.
    /// If the [ColorType] is not supported as a YUVA plane the number of channels is reported as 0
    /// and the [DataType] returned should be ignored.
    pub fn num_channels_and_data_type(color_type: ColorType) -> (usize, DataType) {
        let mut data_type = DataType::Float16;
        let channels = unsafe {
            sb::C_SkYUVAPixmapInfo_NumChannelsAndDataType(color_type.into_native(), &mut data_type)
        };
        (channels.try_into().unwrap(), data_type)
    }
}

#[cfg(test)]
mod tests {
    use crate::yuva_pixmap_info;

    #[test]
    fn test_data_type_naming() {
        let _ = yuva_pixmap_info::DataType::Float16;
    }
}
