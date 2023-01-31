use crate::{
    interop::DynamicMemoryWStream, matrix::ApplyPerspectiveClip, path_types, prelude::*, scalar,
    Data, Matrix, PathDirection, PathFillType, Point, RRect, Rect, Vector,
};
use skia_bindings::{self as sb, SkPath, SkPath_Iter, SkPath_RawIter};
use std::{fmt, marker::PhantomData, mem::forget, ptr};

#[deprecated(since = "0.25.0", note = "use PathDirection")]
pub use path_types::PathDirection as Direction;

#[deprecated(since = "0.25.0", note = "use PathFillType")]
pub use path_types::PathFillType as FillType;

/// Four oval parts with radii (rx, ry) start at last [`Path`] [`Point`] and ends at (x, y).
/// ArcSize and Direction select one of the four oval parts.
pub use skia_bindings::SkPath_ArcSize as ArcSize;
variant_name!(ArcSize::Small, arc_size_naming);

/// AddPathMode chooses how `add_path()` appends. Adding one [`Path`] to another can extend
/// the last contour or start a new contour.
pub use skia_bindings::SkPath_AddPathMode as AddPathMode;
variant_name!(AddPathMode::Append, append_naming);

/// SegmentMask constants correspond to each drawing Verb type in [`crate::Path`]; for instance, if
/// [`crate::Path`] only contains lines, only the [`crate::path::SegmentMask::LINE`] bit is set.
pub use path_types::PathSegmentMask as SegmentMask;

/// Verb instructs [`Path`] how to interpret one or more [`Point`] and optional conic weight;
/// manage contour, and terminate [`Path`].
pub use skia_bindings::SkPath_Verb as Verb;
variant_name!(Verb::Line, verb_naming);

/// Iterates through verb array, and associated [`Point`] array and conic weight.
/// Provides options to treat open contours as closed, and to ignore
/// degenerate data.
#[repr(C)]
pub struct Iter<'a>(SkPath_Iter, PhantomData<&'a Handle<SkPath>>);

impl NativeAccess<SkPath_Iter> for Iter<'_> {
    fn native(&self) -> &SkPath_Iter {
        &self.0
    }
    fn native_mut(&mut self) -> &mut SkPath_Iter {
        &mut self.0
    }
}

impl Drop for Iter<'_> {
    fn drop(&mut self) {
        unsafe { sb::C_SkPath_Iter_destruct(&mut self.0) }
    }
}

impl Default for Iter<'_> {
    /// Initializes [`Iter`] with an empty [`Path`]. `next()` on [`Iter`] returns
    /// [`Verb::Done`].
    /// Call `set_path` to initialize [`Iter`] at a later time.
    ///
    /// Returns: [`Iter`] of empty [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_Iter_Iter>
    fn default() -> Self {
        Iter(unsafe { SkPath_Iter::new() }, PhantomData)
    }
}

impl fmt::Debug for Iter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iter")
            .field("conic_weight", &self.conic_weight())
            .field("is_close_line", &self.is_close_line())
            .field("is_closed_contour", &self.is_closed_contour())
            .finish()
    }
}

impl Iter<'_> {
    /// Sets [`Iter`] to return elements of verb array, [`Point`] array, and conic weight in
    /// path. If `force_close` is `true`, [`Iter`] will add [`Verb::Line`] and [`Verb::Close`] after each
    /// open contour. path is not altered.
    ///
    /// * `path` - [`Path`] to iterate
    /// * `force_close` - `true` if open contours generate [`Verb::Close`]
    /// Returns: [`Iter`] of path
    ///
    /// example: <https://fiddle.skia.org/c/@Path_Iter_const_SkPath>
    pub fn new(path: &Path, force_close: bool) -> Iter {
        Iter(
            unsafe { SkPath_Iter::new1(path.native(), force_close) },
            PhantomData,
        )
    }

    /// Sets [`Iter`] to return elements of verb array, [`Point`] array, and conic weight in
    /// path. If `force_close` is `true`, [`Iter`] will add [`Verb::Line`] and [`Verb::Close`] after each
    /// open contour. path is not altered.
    ///
    /// * `path` - [`Path`] to iterate
    /// * `force_close` - `true` if open contours generate [`Verb::Close`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_Iter_setPath>
    pub fn set_path(mut self, path: &Path, force_close: bool) -> Iter {
        unsafe {
            self.0.setPath(path.native(), force_close);
        }
        let r = Iter(self.0, PhantomData);
        forget(self);
        r
    }

    /// Returns conic weight if `next()` returned [`Verb::Conic`].
    ///
    /// If `next()` has not been called, or `next()` did not return [`Verb::Conic`],
    /// result is `None`.
    ///
    /// Returns: conic weight for conic [`Point`] returned by `next()`
    pub fn conic_weight(&self) -> Option<scalar> {
        #[allow(clippy::map_clone)]
        self.native()
            .fConicWeights
            .into_option()
            .map(|p| unsafe { *p })
    }

    /// Returns `true` if last [`Verb::Line`] returned by `next()` was generated
    /// by [`Verb::Close`]. When `true`, the end point returned by `next()` is
    /// also the start point of contour.
    ///
    /// If `next()` has not been called, or `next()` did not return [`Verb::Line`],
    /// result is undefined.
    ///
    /// Returns: `true` if last [`Verb::Line`] was generated by [`Verb::Close`]
    pub fn is_close_line(&self) -> bool {
        unsafe { sb::C_SkPath_Iter_isCloseLine(self.native()) }
    }

    /// Returns `true` if subsequent calls to `next()` return [`Verb::Close`] before returning
    /// [`Verb::Move`]. if `true`, contour [`Iter`] is processing may end with [`Verb::Close`], or
    /// [`Iter`] may have been initialized with force close set to `true`.
    ///
    /// Returns: `true` if contour is closed
    ///
    /// example: <https://fiddle.skia.org/c/@Path_Iter_isClosedContour>
    pub fn is_closed_contour(&self) -> bool {
        unsafe { self.native().isClosedContour() }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Verb, Vec<Point>);

    /// Returns next [`Verb`] in verb array, and advances [`Iter`].
    /// When verb array is exhausted, returns [`Verb::Done`].
    ///
    /// Zero to four [`Point`] are stored in pts, depending on the returned [`Verb`].
    ///
    /// * `pts` - storage for [`Point`] data describing returned [`Verb`]
    /// Returns: next [`Verb`] from verb array
    ///
    /// example: <https://fiddle.skia.org/c/@Path_RawIter_next>
    fn next(&mut self) -> Option<Self::Item> {
        let mut points = [Point::default(); Verb::MAX_POINTS];
        let verb = unsafe { self.native_mut().next(points.native_mut().as_mut_ptr()) };
        if verb != Verb::Done {
            Some((verb, points[0..verb.points()].into()))
        } else {
            None
        }
    }
}

#[repr(C)]
#[deprecated(
    since = "0.30.0",
    note = "User Iter instead, RawIter will soon be removed."
)]
pub struct RawIter<'a>(SkPath_RawIter, PhantomData<&'a Handle<SkPath>>);

#[allow(deprecated)]
impl NativeAccess<SkPath_RawIter> for RawIter<'_> {
    fn native(&self) -> &SkPath_RawIter {
        &self.0
    }
    fn native_mut(&mut self) -> &mut SkPath_RawIter {
        &mut self.0
    }
}

#[allow(deprecated)]
impl Drop for RawIter<'_> {
    fn drop(&mut self) {
        unsafe { sb::C_SkPath_RawIter_destruct(&mut self.0) }
    }
}

#[allow(deprecated)]
impl Default for RawIter<'_> {
    fn default() -> Self {
        RawIter(
            construct(|ri| unsafe { sb::C_SkPath_RawIter_Construct(ri) }),
            PhantomData,
        )
    }
}

#[allow(deprecated)]
impl RawIter<'_> {
    pub fn new(path: &Path) -> RawIter {
        RawIter::default().set_path(path)
    }

    pub fn set_path(mut self, path: &Path) -> RawIter {
        unsafe { self.native_mut().setPath(path.native()) }
        let r = RawIter(self.0, PhantomData);
        forget(self);
        r
    }

    pub fn peek(&self) -> Verb {
        unsafe { sb::C_SkPath_RawIter_peek(self.native()) }
    }

    pub fn conic_weight(&self) -> scalar {
        self.native().fConicWeight
    }
}

#[allow(deprecated)]
impl Iterator for RawIter<'_> {
    type Item = (Verb, Vec<Point>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut points = [Point::default(); Verb::MAX_POINTS];

        let verb = unsafe { self.native_mut().next(points.native_mut().as_mut_ptr()) };
        (verb != Verb::Done).if_true_some((verb, points[0..verb.points()].into()))
    }
}

pub type Path = Handle<SkPath>;
unsafe_send_sync!(Path);

impl NativeDrop for SkPath {
    /// Releases ownership of any shared data and deletes data if [`Path`] is sole owner.
    ///
    /// example: <https://fiddle.skia.org/c/@Path_destructor>
    fn drop(&mut self) {
        unsafe { sb::C_SkPath_destruct(self) }
    }
}

impl NativeClone for SkPath {
    /// Constructs a copy of an existing path.
    /// Copy constructor makes two paths identical by value. Internally, path and
    /// the returned result share pointer values. The underlying verb array, [`Point`] array
    /// and weights are copied when modified.
    ///
    /// Creating a [`Path`] copy is very efficient and never allocates memory.
    /// [`Path`] are always copied by value from the interface; the underlying shared
    /// pointers are not exposed.
    ///
    /// * `path` - [`Path`] to copy by value
    /// Returns: copy of [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_copy_const_SkPath>
    fn clone(&self) -> Self {
        unsafe { SkPath::new1(self) }
    }
}

impl NativePartialEq for SkPath {
    /// Compares a and b; returns `true` if [`path::FillType`], verb array, [`Point`] array, and weights
    /// are equivalent.
    ///
    /// * `a` - [`Path`] to compare
    /// * `b` - [`Path`] to compare
    /// Returns: `true` if [`Path`] pair are equivalent
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { sb::C_SkPath_Equals(self, rhs) }
    }
}

impl Default for Handle<SkPath> {
    /// See [`Self::new()`]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Path")
            .field("fill_type", &self.fill_type())
            .field("is_convex", &self.is_convex())
            .field("is_oval", &self.is_oval())
            .field("is_rrect", &self.is_rrect())
            .field("is_empty", &self.is_empty())
            .field("is_last_contour_closed", &self.is_last_contour_closed())
            .field("is_finite", &self.is_finite())
            .field("is_volatile", &self.is_volatile())
            .field("is_line", &self.is_line())
            .field("count_points", &self.count_points())
            .field("count_verbs", &self.count_verbs())
            .field("approximate_bytes_used", &self.approximate_bytes_used())
            .field("bounds", &self.bounds())
            .field("is_rect", &self.is_rect())
            .field("segment_masks", &self.segment_masks())
            .field("generation_id", &self.generation_id())
            .field("is_valid", &self.is_valid())
            .finish()
    }
}

/// [`Path`] contain geometry. [`Path`] may be empty, or contain one or more verbs that
/// outline a figure. [`Path`] always starts with a move verb to a Cartesian coordinate,
/// and may be followed by additional verbs that add lines or curves.
/// Adding a close verb makes the geometry into a continuous loop, a closed contour.
/// [`Path`] may contain any number of contours, each beginning with a move verb.
///
/// [`Path`] contours may contain only a move verb, or may also contain lines,
/// quadratic beziers, conics, and cubic beziers. [`Path`] contours may be open or
/// closed.
///
/// When used to draw a filled area, [`Path`] describes whether the fill is inside or
/// outside the geometry. [`Path`] also describes the winding rule used to fill
/// overlapping contours.
///
/// Internally, [`Path`] lazily computes metrics likes bounds and convexity. Call
/// [`Path::update_bounds_cache`] to make [`Path`] thread safe.
impl Path {
    /// Create a new path with the specified segments.
    ///
    /// The points and weights arrays are read in order, based on the sequence of verbs.
    ///
    /// Move    1 point
    /// Line    1 point
    /// Quad    2 points
    /// Conic   2 points and 1 weight
    /// Cubic   3 points
    /// Close   0 points
    ///
    /// If an illegal sequence of verbs is encountered, or the specified number of points
    /// or weights is not sufficient given the verbs, an empty Path is returned.
    ///
    /// A legal sequence of verbs consists of any number of Contours. A contour always begins
    /// with a Move verb, followed by 0 or more segments: Line, Quad, Conic, Cubic, followed
    /// by an optional Close.
    pub fn new_from(
        points: &[Point],
        verbs: &[u8],
        conic_weights: &[scalar],
        fill_type: FillType,
        is_volatile: impl Into<Option<bool>>,
    ) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_Make(
                path,
                points.native().as_ptr(),
                points.len().try_into().unwrap(),
                verbs.as_ptr(),
                verbs.len().try_into().unwrap(),
                conic_weights.as_ptr(),
                conic_weights.len().try_into().unwrap(),
                fill_type,
                is_volatile.into().unwrap_or(false),
            )
        })
    }

    pub fn rect(rect: impl AsRef<Rect>, dir: impl Into<Option<PathDirection>>) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_Rect(
                path,
                rect.as_ref().native(),
                dir.into().unwrap_or(PathDirection::CW),
            )
        })
    }

    pub fn oval(oval: impl AsRef<Rect>, dir: impl Into<Option<PathDirection>>) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_Oval(
                path,
                oval.as_ref().native(),
                dir.into().unwrap_or(PathDirection::CW),
            )
        })
    }

    pub fn oval_with_start_index(
        oval: impl AsRef<Rect>,
        dir: PathDirection,
        start_index: usize,
    ) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_OvalWithStartIndex(
                path,
                oval.as_ref().native(),
                dir,
                start_index.try_into().unwrap(),
            )
        })
    }

    pub fn circle(
        center: impl Into<Point>,
        radius: scalar,
        dir: impl Into<Option<PathDirection>>,
    ) -> Self {
        let center = center.into();
        Self::construct(|path| unsafe {
            sb::C_SkPath_Circle(
                path,
                center.x,
                center.y,
                radius,
                dir.into().unwrap_or(PathDirection::CW),
            )
        })
    }

    pub fn rrect(rect: impl AsRef<RRect>, dir: impl Into<Option<PathDirection>>) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_RRect(
                path,
                rect.as_ref().native(),
                dir.into().unwrap_or(PathDirection::CW),
            )
        })
    }

    pub fn rrect_with_start_index(
        rect: impl AsRef<RRect>,
        dir: PathDirection,
        start_index: usize,
    ) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_RRectWithStartIndex(
                path,
                rect.as_ref().native(),
                dir,
                start_index.try_into().unwrap(),
            )
        })
    }

    pub fn polygon(
        pts: &[Point],
        is_closed: bool,
        fill_type: impl Into<Option<FillType>>,
        is_volatile: impl Into<Option<bool>>,
    ) -> Self {
        Self::construct(|path| unsafe {
            sb::C_SkPath_Polygon(
                path,
                pts.native().as_ptr(),
                pts.len().try_into().unwrap(),
                is_closed,
                fill_type.into().unwrap_or(FillType::Winding),
                is_volatile.into().unwrap_or(false),
            )
        })
    }

    pub fn line(a: impl Into<Point>, b: impl Into<Point>) -> Self {
        Self::polygon(&[a.into(), b.into()], false, None, None)
    }

    /// Constructs an empty [`Path`]. By default, [`Path`] has no verbs, no [`Point`], and no weights.
    /// FillType is set to `Winding`.
    ///
    /// Returns: empty [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_empty_constructor>
    pub fn new() -> Self {
        Self::construct(|path| unsafe { sb::C_SkPath_Construct(path) })
    }

    /// Returns `true` if [`Path`] contain equal verbs and equal weights.
    /// If [`Path`] contain one or more conics, the weights must match.
    ///
    /// `conic_to()` may add different verbs depending on conic weight, so it is not
    /// trivial to interpolate a pair of [`Path`] containing conics with different
    /// conic weight values.
    ///
    /// * `compare` - [`Path`] to compare
    /// Returns: `true` if [`Path`] verb array and weights are equivalent
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isInterpolatable>
    pub fn is_interpolatable(&self, compare: &Path) -> bool {
        unsafe { self.native().isInterpolatable(compare.native()) }
    }

    /// Interpolates between [`Path`] with [`Point`] array of equal size.
    /// Copy verb array and weights to out, and set out [`Point`] array to a weighted
    /// average of this [`Point`] array and ending [`Point`] array, using the formula:
    /// (Path Point * weight) + ending Point * (1 - weight).
    ///
    /// weight is most useful when between zero (ending [`Point`] array) and
    /// one (this Point_Array); will work with values outside of this
    /// range.
    ///
    /// `interpolate()` returns `false` and leaves out unchanged if [`Point`] array is not
    /// the same size as ending [`Point`] array. Call `is_interpolatable()` to check [`Path`]
    /// compatibility prior to calling interpolate().
    ///
    /// * `ending` - [`Point`] array averaged with this [`Point`] array
    /// * `weight` - contribution of this [`Point`] array, and
    ///                one minus contribution of ending [`Point`] array
    /// * `out` - [`Path`] replaced by interpolated averages
    /// Returns: `true` if [`Path`] contain same number of [`Point`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_interpolate>
    pub fn interpolate(&self, ending: &Path, weight: scalar) -> Option<Path> {
        let mut out = Path::default();
        unsafe {
            self.native()
                .interpolate(ending.native(), weight, out.native_mut())
        }
        .if_true_some(out)
    }

    /// Returns [`PathFillType`], the rule used to fill [`Path`].
    ///
    /// Returns: current [`PathFillType`] setting
    pub fn fill_type(&self) -> PathFillType {
        unsafe { sb::C_SkPath_getFillType(self.native()) }
    }

    /// Sets FillType, the rule used to fill [`Path`]. While there is no check
    /// that ft is legal, values outside of FillType are not supported.
    pub fn set_fill_type(&mut self, ft: PathFillType) -> &mut Self {
        self.native_mut().set_fFillType(ft as _);
        self
    }

    /// Returns if FillType describes area outside [`Path`] geometry. The inverse fill area
    /// extends indefinitely.
    ///
    /// Returns: `true` if FillType is `InverseWinding` or `InverseEvenOdd`
    pub fn is_inverse_fill_type(&self) -> bool {
        self.fill_type().is_inverse()
    }

    /// Replaces FillType with its inverse. The inverse of FillType describes the area
    /// unmodified by the original FillType.
    pub fn toggle_inverse_fill_type(&mut self) -> &mut Self {
        let inverse = self.native().fFillType() ^ 2;
        self.native_mut().set_fFillType(inverse);
        self
    }

    #[deprecated(since = "0.36.0", note = "Removed, use is_convex()")]
    pub fn convexity_type(&self) -> ! {
        panic!("Removed")
    }

    #[deprecated(since = "0.36.0", note = "Removed, use is_convex()")]
    pub fn convexity_type_or_unknown(&self) -> ! {
        panic!("Removed")
    }

    /// Returns `true` if the path is convex. If necessary, it will first compute the convexity.
    pub fn is_convex(&self) -> bool {
        unsafe { self.native().isConvex() }
    }

    /// Returns `true` if this path is recognized as an oval or circle.
    ///
    /// bounds receives bounds of oval.
    ///
    /// bounds is unmodified if oval is not found.
    ///
    /// * `bounds` - storage for bounding [`Rect`] of oval; may be `None`
    /// Returns: `true` if [`Path`] is recognized as an oval or circle
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isOval>
    pub fn is_oval(&self) -> Option<Rect> {
        let mut bounds = Rect::default();
        unsafe { self.native().isOval(bounds.native_mut()) }.if_true_some(bounds)
    }

    /// Returns `true` if path is representable as [`RRect`].
    /// Returns `false` if path is representable as oval, circle, or [`Rect`].
    ///
    /// rrect receives bounds of [`RRect`].
    ///
    /// rrect is unmodified if [`RRect`] is not found.
    ///
    /// * `rrect` - storage for bounding [`Rect`] of [`RRect`]; may be `None`
    /// Returns: `true` if [`Path`] contains only [`RRect`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isRRect>
    pub fn is_rrect(&self) -> Option<RRect> {
        let mut rrect = RRect::default();
        unsafe { self.native().isRRect(rrect.native_mut()) }.if_true_some(rrect)
    }

    /// Sets [`Path`] to its initial state.
    /// Removes verb array, [`Point`] array, and weights, and sets FillType to `Winding`.
    /// Internal storage associated with [`Path`] is released.
    ///
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_reset>
    pub fn reset(&mut self) -> &mut Self {
        unsafe { self.native_mut().reset() };
        self
    }

    /// Sets [`Path`] to its initial state, preserving internal storage.
    /// Removes verb array, [`Point`] array, and weights, and sets FillType to `Winding`.
    /// Internal storage associated with [`Path`] is retained.
    ///
    /// Use `rewind()` instead of `reset()` if [`Path`] storage will be reused and performance
    /// is critical.
    ///
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_rewind>
    ///
    pub fn rewind(&mut self) -> &mut Self {
        unsafe { self.native_mut().rewind() };
        self
    }

    /// Returns if [`Path`] is empty.
    /// Empty [`Path`] may have FillType but has no [`Point`], [`Verb`], or conic weight.
    /// [`Path::default()`] constructs empty [`Path`]; `reset()` and `rewind()` make [`Path`] empty.
    ///
    /// Returns: `true` if the path contains no [`Verb`] array
    pub fn is_empty(&self) -> bool {
        unsafe { self.native().isEmpty() }
    }

    /// Returns if contour is closed.
    /// Contour is closed if [`Path`] [`Verb`] array was last modified by `close()`. When stroked,
    /// closed contour draws [`crate::paint::Join`] instead of [`crate::paint::Cap`] at first and last [`Point`].
    ///
    /// Returns: `true` if the last contour ends with a [`Verb::Close`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isLastContourClosed>
    pub fn is_last_contour_closed(&self) -> bool {
        unsafe { self.native().isLastContourClosed() }
    }

    /// Returns `true` for finite [`Point`] array values between negative SK_ScalarMax and
    /// positive SK_ScalarMax. Returns `false` for any [`Point`] array value of
    /// SK_ScalarInfinity, SK_ScalarNegativeInfinity, or SK_ScalarNaN.
    ///
    /// Returns: `true` if all [`Point`] values are finite
    pub fn is_finite(&self) -> bool {
        unsafe { self.native().isFinite() }
    }

    /// Returns `true` if the path is volatile; it will not be altered or discarded
    /// by the caller after it is drawn. [`Path`] by default have volatile set `false`, allowing
    /// [`crate::Surface`] to attach a cache of data which speeds repeated drawing. If `true`, [`crate::Surface`]
    /// may not speed repeated drawing.
    ///
    /// Returns: `true` if caller will alter [`Path`] after drawing
    pub fn is_volatile(&self) -> bool {
        self.native().fIsVolatile() != 0
    }

    /// Specifies whether [`Path`] is volatile; whether it will be altered or discarded
    /// by the caller after it is drawn. [`Path`] by default have volatile set `false`, allowing
    /// `BaseDevice` to attach a cache of data which speeds repeated drawing.
    ///
    /// Mark temporary paths, discarded or modified after use, as volatile
    /// to inform `BaseDevice` that the path need not be cached.
    ///
    /// Mark animating [`Path`] volatile to improve performance.
    /// Mark unchanging [`Path`] non-volatile to improve repeated rendering.
    ///
    /// raster surface [`Path`] draws are affected by volatile for some shadows.
    /// GPU surface [`Path`] draws are affected by volatile for some shadows and concave geometries.
    ///
    /// * `is_volatile` - `true` if caller will alter [`Path`] after drawing
    /// Returns: reference to [`Path`]
    pub fn set_is_volatile(&mut self, is_volatile: bool) -> &mut Self {
        self.native_mut().set_fIsVolatile(is_volatile as _);
        self
    }

    /// Tests if line between [`Point`] pair is degenerate.
    /// Line with no length or that moves a very short distance is degenerate; it is
    /// treated as a point.
    ///
    /// exact changes the equality test. If `true`, returns `true` only if p1 equals p2.
    /// If `false`, returns `true` if p1 equals or nearly equals p2.
    ///
    /// * `p1` - line start point
    /// * `p2` - line end point
    /// * `exact` - if `false`, allow nearly equals
    /// Returns: `true` if line is degenerate; its length is effectively zero
    ///
    /// example: <https://fiddle.skia.org/c/@Path_IsLineDegenerate>
    pub fn is_line_degenerate(p1: impl Into<Point>, p2: impl Into<Point>, exact: bool) -> bool {
        unsafe { SkPath::IsLineDegenerate(p1.into().native(), p2.into().native(), exact) }
    }

    /// Tests if quad is degenerate.
    /// Quad with no length or that moves a very short distance is degenerate; it is
    /// treated as a point.
    ///
    /// * `p1` - quad start point
    /// * `p2` - quad control point
    /// * `p3` - quad end point
    /// * `exact` - if `true`, returns `true` only if p1, p2, and p3 are equal;
    ///               if `false`, returns `true` if p1, p2, and p3 are equal or nearly equal
    /// Returns: `true` if quad is degenerate; its length is effectively zero
    pub fn is_quad_degenerate(
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
        exact: bool,
    ) -> bool {
        unsafe {
            SkPath::IsQuadDegenerate(
                p1.into().native(),
                p2.into().native(),
                p3.into().native(),
                exact,
            )
        }
    }

    /// Tests if cubic is degenerate.
    /// Cubic with no length or that moves a very short distance is degenerate; it is
    /// treated as a point.
    ///
    /// * `p1` - cubic start point
    /// * `p2` - cubic control point 1
    /// * `p3` - cubic control point 2
    /// * `p4` - cubic end point
    /// * `exact` - if `true`, returns `true` only if p1, p2, p3, and p4 are equal;
    ///               if `false`, returns `true` if p1, p2, p3, and p4 are equal or nearly equal
    /// Returns: `true` if cubic is degenerate; its length is effectively zero
    pub fn is_cubic_degenerate(
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
        p4: impl Into<Point>,
        exact: bool,
    ) -> bool {
        unsafe {
            SkPath::IsCubicDegenerate(
                p1.into().native(),
                p2.into().native(),
                p3.into().native(),
                p4.into().native(),
                exact,
            )
        }
    }

    /// Returns `true` if [`Path`] contains only one line;
    /// [`Verb`] array has two entries: [`Verb::Move`], [`Verb::Line`].
    /// If [`Path`] contains one line and line is not `None`, line is set to
    /// line start point and line end point.
    /// Returns `false` if [`Path`] is not one line; line is unaltered.
    ///
    /// * `line` - storage for line. May be `None`
    /// Returns: `true` if [`Path`] contains exactly one line
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isLine>
    pub fn is_line(&self) -> Option<(Point, Point)> {
        let mut line = [Point::default(); 2];
        unsafe { self.native().isLine(line.native_mut().as_mut_ptr()) }
            .if_true_some((line[0], line[1]))
    }

    /// Returns the number of points in [`Path`].
    /// [`Point`] count is initially zero.
    ///
    /// Returns: [`Path`] [`Point`] array length
    ///
    /// example: <https://fiddle.skia.org/c/@Path_countPoints>
    pub fn count_points(&self) -> usize {
        unsafe { self.native().countPoints().try_into().unwrap() }
    }

    /// Returns [`Point`] at index in [`Point`] array. Valid range for index is
    /// 0 to `count_points()` - 1.
    /// Returns (0, 0) if index is out of range.
    ///
    /// * `index` - [`Point`] array element selector
    /// Returns: [`Point`] array value or (0, 0)
    ///
    /// example: <https://fiddle.skia.org/c/@Path_getPoint>
    pub fn get_point(&self, index: usize) -> Option<Point> {
        let p = Point::from_native_c(unsafe {
            sb::C_SkPath_getPoint(self.native(), index.try_into().unwrap())
        });
        // assuming that count_points() is somewhat slow, we
        // check the index when a Point(0,0) is returned.
        if p != Point::default() || index < self.count_points() {
            Some(p)
        } else {
            None
        }
    }

    /// Returns number of points in [`Path`]. Up to max points are copied.
    /// points may be `None`; then, max must be zero.
    /// If max is greater than number of points, excess points storage is unaltered.
    ///
    /// * `points` - storage for [`Path`] [`Point`] array. May be `None`
    /// * `max` - maximum to copy; must be greater than or equal to zero
    /// Returns: [`Path`] [`Point`] array length
    ///
    /// example: <https://fiddle.skia.org/c/@Path_getPoints>
    pub fn get_points(&self, points: &mut [Point]) -> usize {
        unsafe {
            self.native().getPoints(
                points.native_mut().as_mut_ptr(),
                points.len().try_into().unwrap(),
            )
        }
        .try_into()
        .unwrap()
    }

    /// Returns the number of verbs: [`Verb::Move`], [`Verb::Line`], [`Verb::Quad`], [`Verb::Conic`],
    /// [`Verb::Cubic`], and [`Verb::Close`]; added to [`Path`].
    ///
    /// Returns: length of verb array
    ///
    /// example: <https://fiddle.skia.org/c/@Path_countVerbs>
    pub fn count_verbs(&self) -> usize {
        unsafe { self.native().countVerbs() }.try_into().unwrap()
    }

    /// Returns the number of verbs in the path. Up to max verbs are copied. The
    /// verbs are copied as one byte per verb.
    ///
    /// * `verbs` - storage for verbs, may be `None`
    /// * `max` - maximum number to copy into verbs
    /// Returns: the actual number of verbs in the path
    ///
    /// example: <https://fiddle.skia.org/c/@Path_getVerbs>
    pub fn get_verbs(&self, verbs: &mut [u8]) -> usize {
        unsafe {
            self.native()
                .getVerbs(verbs.as_mut_ptr(), verbs.len().try_into().unwrap())
        }
        .try_into()
        .unwrap()
    }

    /// Returns the approximate byte size of the [`Path`] in memory.
    ///
    /// Returns: approximate size
    pub fn approximate_bytes_used(&self) -> usize {
        unsafe { self.native().approximateBytesUsed() }
    }

    /// Exchanges the verb array, [`Point`] array, weights, and [`FillType`] with other.
    /// Cached state is also exchanged. `swap()` internally exchanges pointers, so
    /// it is lightweight and does not allocate memory.
    ///
    /// `swap()` usage has largely been replaced by PartialEq.
    /// [`Path`] do not copy their content on assignment until they are written to,
    /// making assignment as efficient as swap().
    ///
    /// * `other` - [`Path`] exchanged by value
    ///
    /// example: <https://fiddle.skia.org/c/@Path_swap>
    pub fn swap(&mut self, other: &mut Path) -> &mut Self {
        unsafe { self.native_mut().swap(other.native_mut()) }
        self
    }

    /// Returns minimum and maximum axes values of [`Point`] array.
    /// Returns (0, 0, 0, 0) if [`Path`] contains no points. Returned bounds width and height may
    /// be larger or smaller than area affected when [`Path`] is drawn.
    ///
    /// [`Rect`] returned includes all [`Point`] added to [`Path`], including [`Point`] associated with
    /// [`Verb::Move`] that define empty contours.
    ///
    /// Returns: bounds of all [`Point`] in [`Point`] array
    pub fn bounds(&self) -> &Rect {
        Rect::from_native_ref(unsafe { &*sb::C_SkPath_getBounds(self.native()) })
    }

    /// Updates internal bounds so that subsequent calls to `bounds()` are instantaneous.
    /// Unaltered copies of [`Path`] may also access cached bounds through `bounds()`.
    ///
    /// For now, identical to calling `bounds()` and ignoring the returned value.
    ///
    /// Call to prepare [`Path`] subsequently drawn from multiple threads,
    /// to avoid a race condition where each draw separately computes the bounds.
    pub fn update_bounds_cache(&mut self) -> &mut Self {
        self.bounds();
        self
    }

    /// Returns minimum and maximum axes values of the lines and curves in [`Path`].
    /// Returns (0, 0, 0, 0) if [`Path`] contains no points.
    /// Returned bounds width and height may be larger or smaller than area affected
    /// when [`Path`] is drawn.
    ///
    /// Includes [`Point`] associated with [`Verb::Move`] that define empty
    /// contours.
    ///
    /// Behaves identically to `bounds()` when [`Path`] contains
    /// only lines. If [`Path`] contains curves, computed bounds includes
    /// the maximum extent of the quad, conic, or cubic; is slower than `bounds()`;
    /// and unlike `bounds()`, does not cache the result.
    ///
    /// Returns: tight bounds of curves in [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_computeTightBounds>
    pub fn compute_tight_bounds(&self) -> Rect {
        Rect::from_native_c(unsafe { sb::C_SkPath_computeTightBounds(self.native()) })
    }

    /// Returns `true` if rect is contained by [`Path`].
    /// May return `false` when rect is contained by [`Path`].
    ///
    /// For now, only returns `true` if [`Path`] has one contour and is convex.
    /// rect may share points and edges with [`Path`] and be contained.
    /// Returns `true` if rect is empty, that is, it has zero width or height; and
    /// the [`Point`] or line described by rect is contained by [`Path`].
    ///
    /// * `rect` - [`Rect`], line, or [`Point`] checked for containment
    /// Returns: `true` if rect is contained
    ///
    /// example: <https://fiddle.skia.org/c/@Path_conservativelyContainsRect>
    pub fn conservatively_contains_rect(&self, rect: impl AsRef<Rect>) -> bool {
        unsafe {
            self.native()
                .conservativelyContainsRect(rect.as_ref().native())
        }
    }

    /// Grows [`Path`] verb array and [`Point`] array to contain `extra_pt_count` additional [`Point`].
    /// May improve performance and use less memory by
    /// reducing the number and size of allocations when creating [`Path`].
    ///
    /// * `extra_pt_count` - number of additional [`Point`] to allocate
    ///
    /// example: <https://fiddle.skia.org/c/@Path_incReserve>
    pub fn inc_reserve(&mut self, extra_pt_count: usize) -> &mut Self {
        unsafe {
            self.native_mut()
                .incReserve(extra_pt_count.try_into().unwrap())
        }
        self
    }

    #[deprecated(since = "0.37.0", note = "Removed without replacement")]
    pub fn shrink_to_fit(&mut self) -> ! {
        panic!("Removed without replacement");
    }

    /// Adds beginning of contour at [`Point`] (x, y).
    ///
    /// * `x` - x-axis value of contour start
    /// * `y` - y-axis value of contour start
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_moveTo>
    pub fn move_to(&mut self, p: impl Into<Point>) -> &mut Self {
        let p = p.into();
        unsafe {
            self.native_mut().moveTo(p.x, p.y);
        }
        self
    }

    /// Adds beginning of contour relative to last point.
    /// If [`Path`] is empty, starts contour at (dx, dy).
    /// Otherwise, start contour at last point offset by (dx, dy).
    /// Function name stands for "relative move to".
    ///
    /// * `dx` - offset from last point to contour start on x-axis
    /// * `dy` - offset from last point to contour start on y-axis
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_rMoveTo>
    pub fn r_move_to(&mut self, d: impl Into<Vector>) -> &mut Self {
        let d = d.into();
        unsafe {
            self.native_mut().rMoveTo(d.x, d.y);
        }
        self
    }

    /// Adds line from last point to (x, y). If [`Path`] is empty, or last [`Verb`] is
    /// [`Verb::Close`], last point is set to (0, 0) before adding line.
    ///
    /// `line_to()` appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array, if needed.
    /// `line_to()` then appends [`Verb::Line`] to verb array and (x, y) to [`Point`] array.
    ///
    /// * `x` - end of added line on x-axis
    /// * `y` - end of added line on y-axis
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_lineTo>
    pub fn line_to(&mut self, p: impl Into<Point>) -> &mut Self {
        let p = p.into();
        unsafe {
            self.native_mut().lineTo(p.x, p.y);
        }
        self
    }

    /// Adds line from last point to vector (dx, dy). If [`Path`] is empty, or last [`Verb`] is
    /// [`Verb::Close`], last point is set to (0, 0) before adding line.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array, if needed;
    /// then appends [`Verb::Line`] to verb array and line end to [`Point`] array.
    /// Line end is last point plus vector (dx, dy).
    /// Function name stands for "relative line to".
    ///
    /// * `dx` - offset from last point to line end on x-axis
    /// * `dy` - offset from last point to line end on y-axis
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_rLineTo>
    /// example: <https://fiddle.skia.org/c/@Quad_a>
    /// example: <https://fiddle.skia.org/c/@Quad_b>
    pub fn r_line_to(&mut self, d: impl Into<Vector>) -> &mut Self {
        let d = d.into();
        unsafe {
            self.native_mut().rLineTo(d.x, d.y);
        }
        self
    }

    /// Adds quad from last point towards (x1, y1), to (x2, y2).
    /// If [`Path`] is empty, or last [`Verb`] is [`Verb::Close`], last point is set to (0, 0)
    /// before adding quad.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array, if needed;
    /// then appends [`Verb::Quad`] to verb array; and (x1, y1), (x2, y2)
    /// to [`Point`] array.
    ///
    /// * `x1` - control [`Point`] of quad on x-axis
    /// * `y1` - control [`Point`] of quad on y-axis
    /// * `x2` - end [`Point`] of quad on x-axis
    /// * `y2` - end [`Point`] of quad on y-axis
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_quadTo>
    pub fn quad_to(&mut self, p1: impl Into<Point>, p2: impl Into<Point>) -> &mut Self {
        let p1 = p1.into();
        let p2 = p2.into();
        unsafe {
            self.native_mut().quadTo(p1.x, p1.y, p2.x, p2.y);
        }
        self
    }

    /// Adds quad from last point towards vector (dx1, dy1), to vector (dx2, dy2).
    /// If [`Path`] is empty, or last [`Verb`]
    /// is [`Verb::Close`], last point is set to (0, 0) before adding quad.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array,
    /// if needed; then appends [`Verb::Quad`] to verb array; and appends quad
    /// control and quad end to [`Point`] array.
    /// Quad control is last point plus vector (dx1, dy1).
    /// Quad end is last point plus vector (dx2, dy2).
    /// Function name stands for "relative quad to".
    ///
    /// * `dx1` - offset from last point to quad control on x-axis
    /// * `dy1` - offset from last point to quad control on y-axis
    /// * `dx2` - offset from last point to quad end on x-axis
    /// * `dy2` - offset from last point to quad end on y-axis
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Conic_Weight_a>
    /// example: <https://fiddle.skia.org/c/@Conic_Weight_b>
    /// example: <https://fiddle.skia.org/c/@Conic_Weight_c>
    /// example: <https://fiddle.skia.org/c/@Path_rQuadTo>
    pub fn r_quad_to(&mut self, dx1: impl Into<Vector>, dx2: impl Into<Vector>) -> &mut Self {
        let (dx1, dx2) = (dx1.into(), dx2.into());
        unsafe {
            self.native_mut().rQuadTo(dx1.x, dx1.y, dx2.x, dx2.y);
        }
        self
    }

    /// Adds conic from last point towards (x1, y1), to (x2, y2), weighted by w.
    /// If [`Path`] is empty, or last [`Verb`] is [`Verb::Close`], last point is set to (0, 0)
    /// before adding conic.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array, if needed.
    ///
    /// If w is finite and not one, appends [`Verb::Conic`] to verb array;
    /// and (x1, y1), (x2, y2) to [`Point`] array; and w to conic weights.
    ///
    /// If w is one, appends [`Verb::Quad`] to verb array, and
    /// (x1, y1), (x2, y2) to [`Point`] array.
    ///
    /// If w is not finite, appends [`Verb::Line`] twice to verb array, and
    /// (x1, y1), (x2, y2) to [`Point`] array.
    ///
    /// * `x1` - control [`Point`] of conic on x-axis
    /// * `y1` - control [`Point`] of conic on y-axis
    /// * `x2` - end [`Point`] of conic on x-axis
    /// * `y2` - end [`Point`] of conic on y-axis
    /// * `w` - weight of added conic
    /// Returns: reference to [`Path`]
    pub fn conic_to(&mut self, p1: impl Into<Point>, p2: impl Into<Point>, w: scalar) -> &mut Self {
        let p1 = p1.into();
        let p2 = p2.into();
        unsafe {
            self.native_mut().conicTo(p1.x, p1.y, p2.x, p2.y, w);
        }
        self
    }

    /// Adds conic from last point towards vector (dx1, dy1), to vector (dx2, dy2),
    /// weighted by w. If [`Path`] is empty, or last [`Verb`]
    /// is [`Verb::Close`], last point is set to (0, 0) before adding conic.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array,
    /// if needed.
    ///
    /// If w is finite and not one, next appends [`Verb::Conic`] to verb array,
    /// and w is recorded as conic weight; otherwise, if w is one, appends
    /// [`Verb::Quad`] to verb array; or if w is not finite, appends [`Verb::Line`]
    /// twice to verb array.
    ///
    /// In all cases appends [`Point`] control and end to [`Point`] array.
    /// control is last point plus vector (dx1, dy1).
    /// end is last point plus vector (dx2, dy2).
    ///
    /// Function name stands for "relative conic to".
    ///
    /// * `dx1` - offset from last point to conic control on x-axis
    /// * `dy1` - offset from last point to conic control on y-axis
    /// * `dx2` - offset from last point to conic end on x-axis
    /// * `dy2` - offset from last point to conic end on y-axis
    /// * `w` - weight of added conic
    /// Returns: reference to [`Path`]
    pub fn r_conic_to(
        &mut self,
        d1: impl Into<Vector>,
        d2: impl Into<Vector>,
        w: scalar,
    ) -> &mut Self {
        let (d1, d2) = (d1.into(), d2.into());
        unsafe {
            self.native_mut().rConicTo(d1.x, d1.y, d2.x, d2.y, w);
        }
        self
    }

    /// Adds cubic from last point towards (x1, y1), then towards (x2, y2), ending at
    /// (x3, y3). If [`Path`] is empty, or last [`Verb`] is [`Verb::Close`], last point is set to
    /// (0, 0) before adding cubic.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array, if needed;
    /// then appends [`Verb::Cubic`] to verb array; and (x1, y1), (x2, y2), (x3, y3)
    /// to [`Point`] array.
    ///
    /// * `x1` - first control [`Point`] of cubic on x-axis
    /// * `y1` - first control [`Point`] of cubic on y-axis
    /// * `x2` - second control [`Point`] of cubic on x-axis
    /// * `y2` - second control [`Point`] of cubic on y-axis
    /// * `x3` - end [`Point`] of cubic on x-axis
    /// * `y3` - end [`Point`] of cubic on y-axis
    /// Returns: reference to [`Path`]
    pub fn cubic_to(
        &mut self,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        p3: impl Into<Point>,
    ) -> &mut Self {
        let (p1, p2, p3) = (p1.into(), p2.into(), p3.into());
        unsafe {
            self.native_mut()
                .cubicTo(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
        }
        self
    }

    /// Adds cubic from last point towards vector (dx1, dy1), then towards
    /// vector (dx2, dy2), to vector (dx3, dy3).
    /// If [`Path`] is empty, or last [`Verb`]
    /// is [`Verb::Close`], last point is set to (0, 0) before adding cubic.
    ///
    /// Appends [`Verb::Move`] to verb array and (0, 0) to [`Point`] array,
    /// if needed; then appends [`Verb::Cubic`] to verb array; and appends cubic
    /// control and cubic end to [`Point`] array.
    /// Cubic control is last point plus vector (dx1, dy1).
    /// Cubic end is last point plus vector (dx2, dy2).
    /// Function name stands for "relative cubic to".
    ///
    /// * `dx1` - offset from last point to first cubic control on x-axis
    /// * `dy1` - offset from last point to first cubic control on y-axis
    /// * `dx2` - offset from last point to second cubic control on x-axis
    /// * `dy2` - offset from last point to second cubic control on y-axis
    /// * `dx3` - offset from last point to cubic end on x-axis
    /// * `dy3` - offset from last point to cubic end on y-axis
    /// Returns: reference to [`Path`]
    pub fn r_cubic_to(
        &mut self,
        d1: impl Into<Vector>,
        d2: impl Into<Vector>,
        d3: impl Into<Vector>,
    ) -> &mut Self {
        let (d1, d2, d3) = (d1.into(), d2.into(), d3.into());
        unsafe {
            self.native_mut()
                .rCubicTo(d1.x, d1.y, d2.x, d2.y, d3.x, d3.y);
        }
        self
    }

    /// Appends arc to [`Path`]. Arc added is part of ellipse
    /// bounded by oval, from `start_angle` through `sweep_angle`. Both `start_angle` and
    /// `sweep_angle` are measured in degrees, where zero degrees is aligned with the
    /// positive x-axis, and positive sweeps extends arc clockwise.
    ///
    /// `arc_to()` adds line connecting [`Path`] last [`Point`] to initial arc [`Point`] if `force_move_to`
    /// is `false` and [`Path`] is not empty. Otherwise, added contour begins with first point
    /// of arc. Angles greater than -360 and less than 360 are treated modulo 360.
    ///
    /// * `oval` - bounds of ellipse containing arc
    /// * `start_angle` - starting angle of arc in degrees
    /// * `sweep_angle` - sweep, in degrees. Positive is clockwise; treated modulo 360
    /// * `force_move_to` - `true` to start a new contour with arc
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_arcTo>
    pub fn arc_to(
        &mut self,
        oval: impl AsRef<Rect>,
        start_angle: scalar,
        sweep_angle: scalar,
        force_move_to: bool,
    ) -> &mut Self {
        unsafe {
            self.native_mut().arcTo(
                oval.as_ref().native(),
                start_angle,
                sweep_angle,
                force_move_to,
            );
        }
        self
    }

    /// Appends arc to [`Path`], after appending line if needed. Arc is implemented by conic
    /// weighted to describe part of circle. Arc is contained by tangent from
    /// last [`Path`] point to (x1, y1), and tangent from (x1, y1) to (x2, y2). Arc
    /// is part of circle sized to radius, positioned so it touches both tangent lines.
    ///
    /// If last Path Point does not start Arc, `arc_to` appends connecting Line to Path.
    /// The length of Vector from (x1, y1) to (x2, y2) does not affect Arc.
    ///
    /// Arc sweep is always less than 180 degrees. If radius is zero, or if
    /// tangents are nearly parallel, `arc_to` appends Line from last Path Point to (x1, y1).
    ///
    /// `arc_to_tangent` appends at most one Line and one conic.
    /// `arc_to_tangent` implements the functionality of PostScript arct and HTML Canvas `arc_to`.
    ///
    /// * `p1.x` - x-axis value common to pair of tangents
    /// * `p1.y` - y-axis value common to pair of tangents
    /// * `p2.x` - x-axis value end of second tangent
    /// * `p2.y` - y-axis value end of second tangent
    /// * `radius` - distance from arc to circle center
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_arcTo_2_a>
    /// example: <https://fiddle.skia.org/c/@Path_arcTo_2_b>
    /// example: <https://fiddle.skia.org/c/@Path_arcTo_2_c>
    pub fn arc_to_tangent(
        &mut self,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        radius: scalar,
    ) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        unsafe {
            self.native_mut().arcTo1(p1.x, p1.y, p2.x, p2.y, radius);
        }
        self
    }

    /// Appends arc to [`Path`]. Arc is implemented by one or more conics weighted to
    /// describe part of oval with radii (rx, ry) rotated by `x_axis_rotate` degrees. Arc
    /// curves from last [`Path`] [`Point`] to (x, y), choosing one of four possible routes:
    /// clockwise or counterclockwise, and smaller or larger.
    ///
    /// Arc sweep is always less than 360 degrees. `arc_to_rotated()` appends line to (x, y) if
    /// either radii are zero, or if last [`Path`] [`Point`] equals (x, y). `arc_to_rotated()` scales radii
    /// (rx, ry) to fit last [`Path`] [`Point`] and (x, y) if both are greater than zero but
    /// too small.
    ///
    /// `arc_to_rotated()` appends up to four conic curves.
    /// `arc_to_rotated()` implements the functionality of SVG arc, although SVG sweep-flag value
    /// is opposite the integer value of sweep; SVG sweep-flag uses 1 for clockwise,
    /// while [`Direction::CW`] cast to int is zero.
    ///
    /// * `r.x` - radius on x-axis before x-axis rotation
    /// * `r.y` - radius on y-axis before x-axis rotation
    /// * `x_axis_rotate` - x-axis rotation in degrees; positive values are clockwise
    /// * `large_arc` - chooses smaller or larger arc
    /// * `sweep` - chooses clockwise or counterclockwise arc
    /// * `end.x` - end of arc
    /// * `end.y` - end of arc
    /// Returns: reference to [`Path`]

    pub fn arc_to_rotated(
        &mut self,
        r: impl Into<Point>,
        x_axis_rotate: scalar,
        large_arc: ArcSize,
        sweep: PathDirection,
        end: impl Into<Point>,
    ) -> &mut Self {
        let (r, end) = (r.into(), end.into());
        unsafe {
            self.native_mut()
                .arcTo2(r.x, r.y, x_axis_rotate, large_arc, sweep, end.x, end.y);
        }
        self
    }

    /// Appends arc to [`Path`], relative to last [`Path`] [`Point`]. Arc is implemented by one or
    /// more conic, weighted to describe part of oval with radii (r.x, r.y) rotated by
    /// `x_axis_rotate` degrees. Arc curves from last [`Path`] [`Point`] to relative end [`Point`]:
    /// (dx, dy), choosing one of four possible routes: clockwise or
    /// counterclockwise, and smaller or larger. If [`Path`] is empty, the start arc [`Point`]
    /// is (0, 0).
    ///
    /// Arc sweep is always less than 360 degrees. `arc_to()` appends line to end [`Point`]
    /// if either radii are zero, or if last [`Path`] [`Point`] equals end [`Point`].
    /// `arc_to()` scales radii (rx, ry) to fit last [`Path`] [`Point`] and end [`Point`] if both are
    /// greater than zero but too small to describe an arc.
    ///
    /// `arc_to()` appends up to four conic curves.
    /// `arc_to()` implements the functionality of svg arc, although SVG "sweep-flag" value is
    /// opposite the integer value of sweep; SVG "sweep-flag" uses 1 for clockwise, while
    /// [`Direction::CW`] cast to int is zero.
    ///
    /// * `r.x` - radius before x-axis rotation
    /// * `r.y` - radius before x-axis rotation
    /// * `x_axis_rotate` - x-axis rotation in degrees; positive values are clockwise
    /// * `large_arc` - chooses smaller or larger arc
    /// * `sweep` - chooses clockwise or counterclockwise arc
    /// * `d.x` - x-axis offset end of arc from last [`Path`] [`Point`]
    /// * `d.y` - y-axis offset end of arc from last [`Path`] [`Point`]
    /// Returns: reference to [`Path`]
    pub fn r_arc_to_rotated(
        &mut self,
        r: impl Into<Point>,
        x_axis_rotate: scalar,
        large_arc: ArcSize,
        sweep: PathDirection,
        d: impl Into<Point>,
    ) -> &mut Self {
        let (r, d) = (r.into(), d.into());
        unsafe {
            self.native_mut()
                .rArcTo(r.x, r.y, x_axis_rotate, large_arc, sweep, d.x, d.y);
        }
        self
    }

    /// Appends [`Verb::Close`] to [`Path`]. A closed contour connects the first and last [`Point`]
    /// with line, forming a continuous loop. Open and closed contour draw the same
    /// with fill style. With stroke style, open contour draws
    /// [`crate::paint::Cap`] at contour start and end; closed contour draws
    /// [`crate::paint::Join`] at contour start and end.
    ///
    /// `close()` has no effect if [`Path`] is empty or last [`Path`] [`Verb`] is [`Verb::Close`].
    ///
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_close>
    pub fn close(&mut self) -> &mut Self {
        unsafe {
            self.native_mut().close();
        }
        self
    }

    /// Approximates conic with quad array. Conic is constructed from start [`Point`] p0,
    /// control [`Point`] p1, end [`Point`] p2, and weight w.
    /// Quad array is stored in pts; this storage is supplied by caller.
    /// Maximum quad count is 2 to the pow2.
    /// Every third point in array shares last [`Point`] of previous quad and first [`Point`] of
    /// next quad. Maximum pts storage size is given by:
    /// (1 + 2 * (1 << pow2)) * sizeof([`Point`]).
    ///
    /// Returns quad count used the approximation, which may be smaller
    /// than the number requested.
    ///
    /// conic weight determines the amount of influence conic control point has on the curve.
    /// w less than one represents an elliptical section. w greater than one represents
    /// a hyperbolic section. w equal to one represents a parabolic section.
    ///
    /// Two quad curves are sufficient to approximate an elliptical conic with a sweep
    /// of up to 90 degrees; in this case, set pow2 to one.
    ///
    /// * `p0` - conic start [`Point`]
    /// * `p1` - conic control [`Point`]
    /// * `p2` - conic end [`Point`]
    /// * `w` - conic weight
    /// * `pts` - storage for quad array
    /// * `pow2` - quad count, as power of two, normally 0 to 5 (1 to 32 quad curves)
    /// Returns: number of quad curves written to pts
    pub fn convert_conic_to_quads(
        p0: impl Into<Point>,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        w: scalar,
        pts: &mut [Point],
        pow2: usize,
    ) -> Option<usize> {
        let (p0, p1, p2) = (p0.into(), p1.into(), p2.into());
        let max_pts_count = 1 + 2 * (1 << pow2);
        if pts.len() >= max_pts_count {
            Some(unsafe {
                SkPath::ConvertConicToQuads(
                    p0.native(),
                    p1.native(),
                    p2.native(),
                    w,
                    pts.native_mut().as_mut_ptr(),
                    pow2.try_into().unwrap(),
                )
                .try_into()
                .unwrap()
            })
        } else {
            None
        }
    }

    // TODO: return type is probably worth a struct.

    /// Returns `Some(Rect, bool, PathDirection)` if [`Path`] is equivalent to [`Rect`] when filled.
    /// If `false`: rect, `is_closed`, and direction are unchanged.
    /// If `true`: rect, `is_closed`, and direction are written to.
    ///
    /// rect may be smaller than the [`Path`] bounds. [`Path`] bounds may include [`Verb::Move`] points
    /// that do not alter the area drawn by the returned rect.
    ///
    /// Returns: `Some(rect, is_closed, direction)` if [`Path`] contains [`Rect`]
    /// * `rect` - bounds of [`Rect`]
    /// * `is_closed` - set to `true` if [`Path`] is closed
    /// * `direction` - to [`Rect`] direction
    ///
    /// example: <https://fiddle.skia.org/c/@Path_isRect>
    pub fn is_rect(&self) -> Option<(Rect, bool, PathDirection)> {
        let mut rect = Rect::default();
        let mut is_closed = Default::default();
        let mut direction = PathDirection::default();
        unsafe {
            self.native()
                .isRect(rect.native_mut(), &mut is_closed, &mut direction)
        }
        .if_true_some((rect, is_closed, direction))
    }

    /// Adds a new contour to the path, defined by the rect, and wound in the
    /// specified direction. The verbs added to the path will be:
    ///
    /// `Move`, `Line`, `Line`, `Line`, `Close`
    ///
    /// start specifies which corner to begin the contour:
    ///     0: upper-left  corner
    ///     1: upper-right corner
    ///     2: lower-right corner
    ///     3: lower-left  corner
    ///
    /// This start point also acts as the implied beginning of the subsequent,
    /// contour, if it does not have an explicit `move_to`(). e.g.
    ///
    /// `path.add_rect(...)`
    /// // if we don't say `move_to()` here, we will use the rect's start point
    /// `path.line_to`(...)`
    ///
    /// * `rect` - [`Rect`] to add as a closed contour
    /// * `dir` - [`Direction`] to orient the new contour
    /// * `start` - initial corner of [`Rect`] to add
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_addRect_2>
    pub fn add_rect(
        &mut self,
        rect: impl AsRef<Rect>,
        dir_start: Option<(PathDirection, usize)>,
    ) -> &mut Self {
        let dir = dir_start.map(|ds| ds.0).unwrap_or_default();
        let start = dir_start.map(|ds| ds.1).unwrap_or_default();
        unsafe {
            self.native_mut()
                .addRect(rect.as_ref().native(), dir, start.try_into().unwrap())
        };
        self
    }

    /// Adds oval to [`Path`], appending [`Verb::Move`], four [`Verb::Conic`], and [`Verb::Close`].
    /// Oval is upright ellipse bounded by [`Rect`] oval with radii equal to half oval width
    /// and half oval height. Oval begins at start and continues
    /// clockwise if dir is [`Direction::CW`], counterclockwise if dir is [`Direction::CCW`].
    ///
    /// * `oval` - bounds of ellipse added
    /// * `dir` - [`Direction`] to wind ellipse
    /// * `start` - index of initial point of ellipse
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_addOval_2>
    pub fn add_oval(
        &mut self,
        oval: impl AsRef<Rect>,
        dir_start: Option<(PathDirection, usize)>,
    ) -> &mut Self {
        let dir = dir_start.map(|ds| ds.0).unwrap_or_default();
        let start = dir_start.map(|ds| ds.1).unwrap_or_default();
        unsafe {
            self.native_mut()
                .addOval1(oval.as_ref().native(), dir, start.try_into().unwrap())
        };
        self
    }

    /// Adds circle centered at (x, y) of size radius to [`Path`], appending [`Verb::Move`],
    /// four [`Verb::Conic`], and [`Verb::Close`]. Circle begins at: (x + radius, y), continuing
    /// clockwise if dir is [`Direction::CW`], and counterclockwise if dir is [`Direction::CCW`].
    ///
    /// Has no effect if radius is zero or negative.
    ///
    /// * `p` - center of circle
    /// * `radius` - distance from center to edge
    /// * `dir` - [`Direction`] to wind circle
    /// Returns: reference to [`Path`]
    pub fn add_circle(
        &mut self,
        p: impl Into<Point>,
        radius: scalar,
        dir: impl Into<Option<PathDirection>>,
    ) -> &mut Self {
        let p = p.into();
        let dir = dir.into().unwrap_or_default();
        unsafe { self.native_mut().addCircle(p.x, p.y, radius, dir) };
        self
    }

    /// Appends arc to [`Path`], as the start of new contour. Arc added is part of ellipse
    /// bounded by oval, from `start_angle` through `sweep_angle`. Both `start_angle` and
    /// `sweep_angle` are measured in degrees, where zero degrees is aligned with the
    /// positive x-axis, and positive sweeps extends arc clockwise.
    ///
    /// If `sweep_angle` <= -360, or `sweep_angle` >= 360; and `start_angle` modulo 90 is nearly
    /// zero, append oval instead of arc. Otherwise, `sweep_angle` values are treated
    /// modulo 360, and arc may or may not draw depending on numeric rounding.
    ///
    /// * `oval` - bounds of ellipse containing arc
    /// * `start_angle` - starting angle of arc in degrees
    /// * `sweep_angle` - sweep, in degrees. Positive is clockwise; treated modulo 360
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_addArc>
    pub fn add_arc(
        &mut self,
        oval: impl AsRef<Rect>,
        start_angle: scalar,
        sweep_angle: scalar,
    ) -> &mut Self {
        unsafe {
            self.native_mut()
                .addArc(oval.as_ref().native(), start_angle, sweep_angle)
        };
        self
    }

    // Decided to only provide the simpler variant of the two, if radii needs to be specified,
    // add_rrect can be used.

    /// Appends [`RRect`] to [`Path`], creating a new closed contour. [`RRect`] has bounds
    /// equal to rect; each corner is 90 degrees of an ellipse with radii (rx, ry). If
    /// dir is [`Direction::CW`], [`RRect`] starts at top-left of the lower-left corner and
    /// winds clockwise. If dir is [`Direction::CCW`], [`RRect`] starts at the bottom-left
    /// of the upper-left corner and winds counterclockwise.
    ///
    /// If either rx or ry is too large, rx and ry are scaled uniformly until the
    /// corners fit. If rx or ry is less than or equal to zero, `add_round_rect()` appends
    /// [`Rect`] rect to [`Path`].
    ///
    /// After appending, [`Path`] may be empty, or may contain: [`Rect`], oval, or [`RRect`].
    ///
    /// * `rect` - bounds of [`RRect`]
    /// * `rx` - x-axis radius of rounded corners on the [`RRect`]
    /// * `ry` - y-axis radius of rounded corners on the [`RRect`]
    /// * `dir` - [`Direction`] to wind [`RRect`]
    /// Returns: reference to [`Path`]
    pub fn add_round_rect(
        &mut self,
        rect: impl AsRef<Rect>,
        (rx, ry): (scalar, scalar),
        dir: impl Into<Option<PathDirection>>,
    ) -> &mut Self {
        let dir = dir.into().unwrap_or_default();
        unsafe {
            self.native_mut()
                .addRoundRect(rect.as_ref().native(), rx, ry, dir)
        };
        self
    }

    /// Adds rrect to [`Path`], creating a new closed contour. If dir is [`Direction::CW`], rrect
    /// winds clockwise; if dir is [`Direction::CCW`], rrect winds counterclockwise.
    /// start determines the first point of rrect to add.
    ///
    /// * `rrect` - bounds and radii of rounded rectangle
    /// * `dir` - [`PathDirection`] to wind [`RRect`]
    /// * `start` - index of initial point of [`RRect`]
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_addRRect_2>
    pub fn add_rrect(
        &mut self,
        rrect: impl AsRef<RRect>,
        dir_start: Option<(PathDirection, usize)>,
    ) -> &mut Self {
        let dir = dir_start.map(|ds| ds.0).unwrap_or_default();
        let start = dir_start.map(|ds| ds.1).unwrap_or_default();
        unsafe {
            self.native_mut()
                .addRRect1(rrect.as_ref().native(), dir, start.try_into().unwrap())
        };
        self
    }

    /// Adds contour created from line array, adding `pts.len() - 1` line segments.
    /// Contour added starts at `pts[0]`, then adds a line for every additional [`Point`]
    /// in pts slice. If close is `true`, appends [`Verb::Close`] to [`Path`], connecting
    /// `pts[pts.len() - 1]` and `pts[0]`.
    ///
    /// If count is zero, append [`Verb::Move`] to path.
    /// Has no effect if ps.len() is less than one.
    ///
    /// * `pts` - slice of line sharing end and start [`Point`]
    /// * `close` - `true` to add line connecting contour end and start
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_addPoly>
    pub fn add_poly(&mut self, pts: &[Point], close: bool) -> &mut Self {
        unsafe {
            self.native_mut()
                .addPoly(pts.native().as_ptr(), pts.len().try_into().unwrap(), close)
        };
        self
    }

    // TODO: addPoly(initializer_list)

    /// Appends src to [`Path`], offset by `(d.x, d.y)`.
    ///
    /// If mode is [`AddPathMode::Append`], src verb array, [`Point`] array, and conic weights are
    /// added unaltered. If mode is [`AddPathMode::Extend`], add line before appending
    /// verbs, [`Point`], and conic weights.
    ///
    /// * `src` - [`Path`] verbs, [`Point`], and conic weights to add
    /// * `d.x` - offset added to src [`Point`] array x-axis coordinates
    /// * `d.y` - offset added to src [`Point`] array y-axis coordinates
    /// * `mode` - [`AddPathMode::Append`] or [`AddPathMode::Extend`]
    /// Returns: reference to [`Path`]
    pub fn add_path(
        &mut self,
        src: &Path,
        d: impl Into<Vector>,
        mode: impl Into<Option<AddPathMode>>,
    ) -> &mut Self {
        let d = d.into();
        let mode = mode.into().unwrap_or(AddPathMode::Append);
        unsafe { self.native_mut().addPath(src.native(), d.x, d.y, mode) };
        self
    }

    // TODO: rename to add_path_with_matrix() ?

    /// Appends src to [`Path`], transformed by matrix. Transformed curves may have different
    /// verbs, [`Point`], and conic weights.
    ///
    /// If mode is [`AddPathMode::Append`], src verb array, [`Point`] array, and conic weights are
    /// added unaltered. If mode is [`AddPathMode::Extend`], add line before appending
    /// verbs, [`Point`], and conic weights.
    ///
    /// * `src` - [`Path`] verbs, [`Point`], and conic weights to add
    /// * `matrix` - transform applied to src
    /// * `mode` - [`AddPathMode::Append`] or [`AddPathMode::Extend`]
    /// Returns: reference to [`Path`]
    pub fn add_path_matrix(
        &mut self,
        src: &Path,
        matrix: &Matrix,
        mode: impl Into<Option<AddPathMode>>,
    ) -> &mut Self {
        let mode = mode.into().unwrap_or(AddPathMode::Append);
        unsafe {
            self.native_mut()
                .addPath1(src.native(), matrix.native(), mode)
        };
        self
    }

    /// Appends src to [`Path`], from back to front.
    /// Reversed src always appends a new contour to [`Path`].
    ///
    /// * `src` - [`Path`] verbs, [`Point`], and conic weights to add
    /// Returns: reference to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_reverseAddPath>
    pub fn reverse_add_path(&mut self, src: &Path) -> &mut Self {
        unsafe { self.native_mut().reverseAddPath(src.native()) };
        self
    }

    /// Offsets [`Point`] array by `(d.x, d.y)`.
    ///
    /// * `dx` - offset added to [`Point`] array x-axis coordinates
    /// * `dy` - offset added to [`Point`] array y-axis coordinates
    /// Returns: overwritten, translated copy of [`Path`]; may be `None`
    ///
    /// example: <https://fiddle.skia.org/c/@Path_offset>
    #[must_use]
    pub fn with_offset(&self, d: impl Into<Vector>) -> Path {
        let d = d.into();
        let mut path = Path::default();
        unsafe { self.native().offset(d.x, d.y, path.native_mut()) };
        path
    }

    /// Offsets [`Point`] array by `(d.x, d.y)`. [`Path`] is replaced by offset data.
    ///
    /// * `d.x` - offset added to [`Point`] array x-axis coordinates
    /// * `d.y` - offset added to [`Point`] array y-axis coordinates
    pub fn offset(&mut self, d: impl Into<Vector>) -> &mut Self {
        let d = d.into();
        unsafe {
            let self_ptr = self.native_mut() as *mut _;
            self.native().offset(d.x, d.y, self_ptr)
        };
        self
    }

    /// Transforms verb array, [`Point`] array, and weight by matrix.
    /// transform may change verbs and increase their number.
    ///
    /// * `matrix` - [`Matrix`] to apply to [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_transform>
    #[must_use]
    pub fn with_transform(&self, matrix: &Matrix) -> Path {
        self.with_transform_with_perspective_clip(matrix, ApplyPerspectiveClip::Yes)
    }

    /// Transforms verb array, [`Point`] array, and weight by matrix.
    /// transform may change verbs and increase their number.
    ///
    /// * `matrix` - [`Matrix`] to apply to [`Path`]
    /// * `pc` - whether to apply perspective clipping
    ///
    /// example: <https://fiddle.skia.org/c/@Path_transform>
    #[must_use]
    pub fn with_transform_with_perspective_clip(
        &self,
        matrix: &Matrix,
        perspective_clip: ApplyPerspectiveClip,
    ) -> Path {
        let mut path = Path::default();
        unsafe {
            self.native()
                .transform(matrix.native(), path.native_mut(), perspective_clip)
        };
        path
    }

    /// Transforms verb array, [`Point`] array, and weight by matrix.
    /// transform may change verbs and increase their number.
    ///
    /// * `matrix` - [`Matrix`] to apply to [`Path`]
    pub fn transform(&mut self, matrix: &Matrix) -> &mut Self {
        self.transform_with_perspective_clip(matrix, ApplyPerspectiveClip::Yes)
    }

    /// Transforms verb array, [`Point`] array, and weight by matrix.
    /// transform may change verbs and increase their number.
    ///
    /// * `matrix` - [`Matrix`] to apply to [`Path`]
    /// * `pc` - whether to apply perspective clipping
    pub fn transform_with_perspective_clip(
        &mut self,
        matrix: &Matrix,
        pc: ApplyPerspectiveClip,
    ) -> &mut Self {
        let self_ptr = self.native_mut() as *mut _;
        unsafe { self.native().transform(matrix.native(), self_ptr, pc) };
        self
    }

    #[must_use]
    pub fn make_transform(
        &mut self,
        m: &Matrix,
        pc: impl Into<Option<ApplyPerspectiveClip>>,
    ) -> Path {
        self.with_transform_with_perspective_clip(m, pc.into().unwrap_or(ApplyPerspectiveClip::Yes))
    }

    #[must_use]
    pub fn make_scale(&mut self, (sx, sy): (scalar, scalar)) -> Path {
        self.make_transform(&Matrix::scale((sx, sy)), ApplyPerspectiveClip::No)
    }

    /// Returns last point on [`Path`]. Returns `None` if [`Point`] array is empty,
    /// storing `(0, 0)` if `last_pt` is not `None`.
    ///
    /// Returns final [`Point`] in [`Point`] array; may be `None`
    /// Returns: `Some` if [`Point`] array contains one or more [`Point`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_getLastPt>
    pub fn last_pt(&self) -> Option<Point> {
        let mut last_pt = Point::default();
        unsafe { self.native().getLastPt(last_pt.native_mut()) }.if_true_some(last_pt)
    }

    /// Sets the last point on the path. If [`Point`] array is empty, append [`Verb::Move`] to
    /// verb array and append p to [`Point`] array.
    ///
    /// * `p` - set value of last point
    pub fn set_last_pt(&mut self, p: impl Into<Point>) -> &mut Self {
        let p = p.into();
        unsafe { self.native_mut().setLastPt(p.x, p.y) };
        self
    }

    /// Returns a mask, where each set bit corresponds to a [`SegmentMask`] constant
    /// if [`Path`] contains one or more verbs of that type.
    /// Returns zero if [`Path`] contains no lines, or curves: quads, conics, or cubics.
    ///
    /// `segment_masks()` returns a cached result; it is very fast.
    ///
    /// Returns: [`SegmentMask`] bits or zero
    pub fn segment_masks(&self) -> SegmentMask {
        SegmentMask::from_bits_truncate(unsafe { self.native().getSegmentMasks() })
    }

    /// Returns `true` if the point `(p.x, p.y)` is contained by [`Path`], taking into
    /// account [`FillType`].
    ///
    /// * `p.x` - x-axis value of containment test
    /// * `p.y` - y-axis value of containment test
    /// Returns: `true` if [`Point`] is in [`Path`]
    ///
    /// example: <https://fiddle.skia.org/c/@Path_contains>
    pub fn contains(&self, p: impl Into<Point>) -> bool {
        let p = p.into();
        unsafe { self.native().contains(p.x, p.y) }
    }

    /// Writes text representation of [`Path`] to [`Data`].
    /// Set `dump_as_hex` `true` to generate exact binary representations
    /// of floating point numbers used in [`Point`] array and conic weights.
    ///
    /// * `dump_as_hex` - `true` if scalar values are written as hexadecimal
    ///
    /// example: <https://fiddle.skia.org/c/@Path_dump>
    pub fn dump_as_data(&self, dump_as_hex: bool) -> Data {
        let mut stream = DynamicMemoryWStream::new();
        unsafe {
            self.native()
                .dump(stream.native_mut().base_mut(), dump_as_hex);
        }
        stream.detach_as_data()
    }

    /// See [`Path::dump_as_data()`]
    pub fn dump(&self) {
        unsafe { self.native().dump(ptr::null_mut(), false) }
    }

    /// See [`Path::dump_as_data()`]
    pub fn dump_hex(&self) {
        unsafe { self.native().dump(ptr::null_mut(), true) }
    }

    // Like [`Path::dump()`], but outputs for the [`Path::make()`] factory
    pub fn dump_arrays_as_data(&self, dump_as_hex: bool) -> Data {
        let mut stream = DynamicMemoryWStream::new();
        unsafe {
            self.native()
                .dumpArrays(stream.native_mut().base_mut(), dump_as_hex);
        }
        stream.detach_as_data()
    }

    // Like [`Path::dump()`], but outputs for the [`Path::make()`] factory
    pub fn dump_arrays(&self) {
        unsafe { self.native().dumpArrays(ptr::null_mut(), false) }
    }

    // TODO: writeToMemory()?

    /// Writes [`Path`] to buffer, returning the buffer written to, wrapped in [`Data`].
    ///
    /// `serialize()` writes [`FillType`], verb array, [`Point`] array, conic weight, and
    /// additionally writes computed information like convexity and bounds.
    ///
    /// `serialize()` should only be used in concert with `read_from_memory`().
    /// The format used for [`Path`] in memory is not guaranteed.
    ///
    /// Returns: [`Path`] data wrapped in [`Data`] buffer
    ///
    /// example: <https://fiddle.skia.org/c/@Path_serialize>
    pub fn serialize(&self) -> Data {
        Data::from_ptr(unsafe { sb::C_SkPath_serialize(self.native()) }).unwrap()
    }

    // TODO: readFromMemory()?

    pub fn deserialize(data: &Data) -> Option<Path> {
        let mut path = Path::default();
        let bytes = data.as_bytes();
        unsafe {
            path.native_mut()
                .readFromMemory(bytes.as_ptr() as _, bytes.len())
                > 0
        }
        .if_true_some(path)
    }
    /// (See Skia bug 1762.)
    /// Returns a non-zero, globally unique value. A different value is returned
    /// if verb array, [`Point`] array, or conic weight changes.
    ///
    /// Setting [`FillType`] does not change generation identifier.
    ///
    /// Each time the path is modified, a different generation identifier will be returned.
    /// [`FillType`] does affect generation identifier on Android framework.
    ///
    /// Returns: non-zero, globally unique value
    ///
    /// example: <https://fiddle.skia.org/c/@Path_getGenerationID>
    pub fn generation_id(&self) -> u32 {
        unsafe { self.native().getGenerationID() }
    }

    /// Returns if [`Path`] data is consistent. Corrupt [`Path`] data is detected if
    /// internal values are out of range or internal storage does not match
    /// array dimensions.
    ///
    /// Returns: `true` if [`Path`] data is consistent
    pub fn is_valid(&self) -> bool {
        unsafe { self.native().isValid() }
    }
}

#[test]
fn test_get_points() {
    let mut p = Path::new();
    p.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), None);
    let points_count = p.count_points();
    let mut points = vec![Point::default(); points_count];
    let count_returned = p.get_points(&mut points);
    assert_eq!(count_returned, points.len());
    assert_eq!(count_returned, 4);
}

#[test]
fn test_fill_type() {
    let mut p = Path::default();
    assert_eq!(p.fill_type(), PathFillType::Winding);
    p.set_fill_type(PathFillType::EvenOdd);
    assert_eq!(p.fill_type(), PathFillType::EvenOdd);
    assert!(!p.is_inverse_fill_type());
    p.toggle_inverse_fill_type();
    assert_eq!(p.fill_type(), PathFillType::InverseEvenOdd);
    assert!(p.is_inverse_fill_type());
}

#[test]
fn test_is_volatile() {
    let mut p = Path::default();
    assert!(!p.is_volatile());
    p.set_is_volatile(true);
    assert!(p.is_volatile());
}

#[test]
fn test_path_rect() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    let path = Path::rect(r, None);
    assert_eq!(*path.bounds(), r);
}
