pub extern crate binaryen_sys;

pub use binaryen_sys as ffi;

use std::rc::Rc;
use std::os::raw::c_char;
use std::ptr;

mod to_cstr;
mod relooper;

use to_cstr::*;
pub use relooper::*;

struct InnerModule {
    raw: ffi::BinaryenModuleRef,
}

impl Drop for InnerModule {
    fn drop(&mut self) {
        unsafe { ffi::BinaryenModuleDispose(self.raw) }
    }
}

pub struct Module {
    inner: Rc<InnerModule>,
}

impl Module {
    pub fn new() -> Module {
        let raw = unsafe { ffi::BinaryenModuleCreate() };
        Module::from_raw(raw)
    }

    pub fn read(wasm_buf: &[u8]) -> Module {
        let raw =
            unsafe { ffi::BinaryenModuleRead(wasm_buf.as_ptr() as *mut c_char, wasm_buf.len()) };
        Module::from_raw(raw)
    }

    pub fn from_raw(raw: ffi::BinaryenModuleRef) -> Module {
        Module {
            inner: Rc::new(InnerModule { raw }),
        }
    }

    pub fn trace(&self) {
        unsafe {
            ffi::BinaryenSetAPITracing(1);
        }
    }

    pub fn auto_drop(&self) {
        unsafe {
            ffi::BinaryenModuleAutoDrop(self.inner.raw);
        }
    }

    pub fn optimize(&self) {
        unsafe { ffi::BinaryenModuleOptimize(self.inner.raw) }
    }

    pub fn is_valid(&self) -> bool {
        unsafe { ffi::BinaryenModuleValidate(self.inner.raw) == 1 }
    }

    pub fn print(&self) {
        unsafe { ffi::BinaryenModulePrint(self.inner.raw) }
    }

    pub fn set_start(&self, fn_ref: &FnRef) {
        unsafe {
            ffi::BinaryenSetStart(self.inner.raw, fn_ref.inner);
        }
    }

    pub fn write(&self) -> Vec<u8> {
        const MAX_CAPACITY: usize = 1024 * 1024 * 2;
        let mut buf: Vec<u8> = Vec::with_capacity(MAX_CAPACITY);
        unsafe {
            let written = ffi::BinaryenModuleWrite(
                self.inner.raw,
                buf.as_mut_ptr() as *mut c_char,
                MAX_CAPACITY,
            );
            if written == buf.capacity() {
                // TODO:
                panic!("unimplemented");
            }
            buf.set_len(written);
        };
        buf.shrink_to_fit();
        buf
    }

    pub fn set_memory<'a, I, P, N>(&self, initial: u32, maximal: u32, name: Option<N>, segments: I)
    where
        I: IntoIterator<Item = Segment<'a>>,
        N: ToCStr<P>,
    {
        let name = to_cstr_stash_option(name);
        unsafe {
            let (datas_and_sizes, mut offset_exprs): (Vec<_>, Vec<_>) = segments
                .into_iter()
                .map(|Segment { data, offset_expr }| {
                    ((data.as_ptr(), data.len() as u32), offset_expr.into_raw())
                })
                .unzip();
            let (mut datas, mut sizes): (Vec<_>, Vec<_>) = datas_and_sizes.into_iter().unzip();
            let segments_count = offset_exprs.len();
            ffi::BinaryenSetMemory(
                self.inner.raw,
                initial,
                maximal,
                name.as_ptr(),
                datas.as_mut_ptr() as *mut *const c_char,
                offset_exprs.as_mut_ptr(),
                sizes.as_mut_ptr(),
                segments_count as _,
            )
        }
    }

    pub fn relooper(&self) -> Relooper {
        Relooper::new(Rc::clone(&self.inner))
    }

    pub fn add_fn_type<P, N: ToCStr<P>>(
        &self,
        name: Option<N>,
        param_tys: &[ValueTy],
        result_ty: Ty,
    ) -> FnType {
        let name = to_cstr_stash_option(name);
        let raw = unsafe {
            let mut param_tys_raw = param_tys
                .iter()
                .cloned()
                .map(|ty| ty.into())
                .collect::<Vec<_>>();
            ffi::BinaryenAddFunctionType(
                self.inner.raw,
                name.as_ptr(),
                result_ty.into(),
                param_tys_raw.as_mut_ptr(),
                param_tys_raw.len() as _,
            )
        };
        FnType { raw }
    }

    pub fn add_fn<P, N: ToCStr<P>>(
        &self,
        name: N,
        fn_ty: &FnType,
        var_tys: &[ValueTy],
        body: Expr,
    ) -> FnRef {
        let name = name.to_cstr_stash();
        let inner = unsafe {
            let mut var_tys_raw = var_tys
                .iter()
                .cloned()
                .map(|ty| ty.into())
                .collect::<Vec<_>>();
            ffi::BinaryenAddFunction(
                self.inner.raw,
                name.as_ptr(),
                fn_ty.raw,
                var_tys_raw.as_mut_ptr(),
                var_tys_raw.len() as _,
                body.into_raw(),
            )
        };
        FnRef { inner }
    }

    pub fn add_global<P, N: ToCStr<P>>(&self, name: N, ty: ValueTy, mutable: bool, init: Expr) {
        let name = name.to_cstr_stash();
        unsafe {
            ffi::BinaryenAddGlobal(
                self.inner.raw,
                name.as_ptr(),
                ty.into(),
                mutable as i8,
                init.into_raw(),
            );
        }
    }

    pub fn add_import<P1, N1: ToCStr<P1>, P2, N2: ToCStr<P2>, P3, N3: ToCStr<P3>>(
        &self,
        internal_name: N1,
        external_module_name: N2,
        external_base_name: N3,
        fn_ty: &FnType,
    ) {
        let internal_name = internal_name.to_cstr_stash();
        let external_module_name = external_module_name.to_cstr_stash();
        let external_base_name = external_base_name.to_cstr_stash();
        unsafe {
            ffi::BinaryenAddImport(
                self.inner.raw,
                internal_name.as_ptr(),
                external_module_name.as_ptr(),
                external_base_name.as_ptr(),
                fn_ty.raw,
            );
        }
    }

    pub fn add_export<P1, N1: ToCStr<P1>, P2, N2: ToCStr<P2>>(
        &self,
        internal_name: N1,
        external_name: N2,
    ) {
        let internal_name = internal_name.to_cstr_stash();
        let external_name = external_name.to_cstr_stash();
        unsafe {
            ffi::BinaryenAddExport(
                self.inner.raw,
                internal_name.as_ptr(),
                external_name.as_ptr(),
            );
        }
    }

    pub fn if_(&self, condition: Expr, if_true: Expr, if_false: Option<Expr>) -> Expr {
        let raw_expr = unsafe {
            let raw_if_true = if_true.into_raw();
            let raw_if_false = if_false.map_or(ptr::null_mut(), |v| v.into_raw());
            ffi::BinaryenIf(
                self.inner.raw,
                condition.into_raw(),
                raw_if_true,
                raw_if_false,
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn loop_<P, N: ToCStr<P>>(&self, name: N, body: Expr) -> Expr {
        let name = name.to_cstr_stash();
        let raw_expr = unsafe { ffi::BinaryenLoop(self.inner.raw, name.as_ptr(), body.into_raw()) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn break_<P, N: ToCStr<P>>(
        &self,
        name: N,
        condition: Option<Expr>,
        value: Option<Expr>,
    ) -> Expr {
        let name = name.to_cstr_stash();
        let raw_expr = unsafe {
            let raw_condition = condition.map_or(ptr::null_mut(), |v| v.into_raw());
            let raw_value = value.map_or(ptr::null_mut(), |v| v.into_raw());
            ffi::BinaryenBreak(self.inner.raw, name.as_ptr(), raw_condition, raw_value)
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn switch<P1, N1: ToCStr<P1>, P2, N2: ToCStr<P2>>(
        &self,
        names: Vec<N1>,
        default_name: N2,
        condition: Expr,
        value: Option<Expr>,
    ) -> Expr {
        let default_name = default_name.to_cstr_stash();
        let raw_expr = unsafe {
            let (_storage, mut name_ptrs): (Vec<_>, Vec<_>) = names
                .into_iter()
                .map(|x| x.to_cstr_stash())
                .map(|Stash { storage, ptr }| (storage, ptr))
                .unzip();
            let raw_condition = condition.into_raw();
            let raw_value = value.map_or(ptr::null_mut(), |v| v.into_raw());
            ffi::BinaryenSwitch(
                self.inner.raw,
                name_ptrs.as_mut_ptr(),
                name_ptrs.len() as _,
                default_name.as_ptr(),
                raw_condition,
                raw_value,
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    // TODO: undefined ty?
    // https://github.com/WebAssembly/binaryen/blob/master/src/binaryen-c.h#L272
    pub fn block<P, N, I>(&self, name: Option<N>, children: I, ty: Ty) -> Expr
    where
        I: IntoIterator<Item = Expr>,
        N: ToCStr<P>,
    {
        let name = to_cstr_stash_option(name);
        let raw_expr = unsafe {
            let mut children_raw: Vec<_> = children.into_iter().map(|ty| ty.into_raw()).collect();
            ffi::BinaryenBlock(
                self.inner.raw,
                name.as_ptr(),
                children_raw.as_mut_ptr(),
                children_raw.len() as _,
                ty.into(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn const_(&self, literal: Literal) -> Expr {
        let raw_expr = unsafe { ffi::BinaryenConst(self.inner.raw, literal.into()) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn load(
        &self,
        bytes: u32,
        signed: bool,
        offset: u32,
        align: u32,
        ty: ValueTy,
        ptr: Expr,
    ) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenLoad(
                self.inner.raw,
                bytes,
                signed as i8,
                offset,
                align,
                ty.into(),
                ptr.into_raw(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn store(
        &self,
        bytes: u32,
        offset: u32,
        align: u32,
        ptr: Expr,
        value: Expr,
        ty: ValueTy,
    ) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenStore(
                self.inner.raw,
                bytes,
                offset,
                align,
                ptr.into_raw(),
                value.into_raw(),
                ty.into(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn get_global<P, N: ToCStr<P>>(&self, name: N, ty: ValueTy) -> Expr {
        let name = name.to_cstr_stash();
        let raw_expr = unsafe { ffi::BinaryenGetGlobal(self.inner.raw, name.as_ptr(), ty.into()) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn set_global<P, N: ToCStr<P>>(&self, name: N, value: Expr) -> Expr {
        let name = name.to_cstr_stash();
        let raw_expr =
            unsafe { ffi::BinaryenSetGlobal(self.inner.raw, name.as_ptr(), value.into_raw()) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn get_local(&self, index: u32, ty: ValueTy) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenGetLocal(self.inner.raw, index as ffi::BinaryenIndex, ty.into())
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn set_local(&self, index: u32, value: Expr) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenSetLocal(
                self.inner.raw,
                index as ffi::BinaryenIndex,
                value.into_raw(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn tee_local(&self, index: u32, value: Expr) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenTeeLocal(
                self.inner.raw,
                index as ffi::BinaryenIndex,
                value.into_raw(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn return_(&self, value: Option<Expr>) -> Expr {
        let raw_expr = unsafe {
            let raw_value = value.map_or(ptr::null_mut(), |v| v.into_raw());
            ffi::BinaryenReturn(self.inner.raw, raw_value)
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn call<P, N, I>(&self, name: N, operands: I) -> Expr
    where
        N: ToCStr<P>,
        I: IntoIterator<Item = Expr>,
    {
        let name = name.to_cstr_stash();
        let raw_expr = unsafe {
            let mut operands_raw: Vec<_> = operands.into_iter().map(|ty| ty.into_raw()).collect();
            ffi::BinaryenCall(
                self.inner.raw,
                name.as_ptr(),
                operands_raw.as_mut_ptr(),
                operands_raw.len() as _,
                ffi::BinaryenNone(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn call_indirect<P, N, I>(&self, target: Expr, operands: I, ty_name: N) -> Expr
    where
        N: ToCStr<P>,
        I: IntoIterator<Item = Expr>,
    {
        let ty_name = ty_name.to_cstr_stash();
        let raw_expr = unsafe {
            let mut operands_raw: Vec<_> = operands.into_iter().map(|ty| ty.into_raw()).collect();
            ffi::BinaryenCallIndirect(
                self.inner.raw,
                target.into_raw(),
                operands_raw.as_mut_ptr(),
                operands_raw.len() as _,
                ty_name.as_ptr(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn call_import<P, N, I>(&self, name: N, operands: I, ty: Ty) -> Expr
    where
        N: ToCStr<P>,
        I: IntoIterator<Item = Expr>,
    {
        let name = name.to_cstr_stash();
        let raw_expr = unsafe {
            let mut operands_raw: Vec<_> = operands.into_iter().map(|ty| ty.into_raw()).collect();
            ffi::BinaryenCallImport(
                self.inner.raw,
                name.as_ptr(),
                operands_raw.as_mut_ptr(),
                operands_raw.len() as _,
                ty.into(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn binary(&self, op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenBinary(self.inner.raw, op.into(), lhs.into_raw(), rhs.into_raw())
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn unary(&self, op: UnaryOp, val: Expr) -> Expr {
        let raw_expr = unsafe { ffi::BinaryenUnary(self.inner.raw, op.into(), val.into_raw()) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn host<P, N, I>(&self, op: HostOp, name: Option<N>, operands: I) -> Expr
    where
        N: ToCStr<P>,
        I: IntoIterator<Item = Expr>,
    {
        let name = to_cstr_stash_option(name);
        let raw_expr = unsafe {
            let mut operands_raw: Vec<_> = operands.into_iter().map(|ty| ty.into_raw()).collect();
            ffi::BinaryenHost(
                self.inner.raw,
                op.into(),
                name.as_ptr(),
                operands_raw.as_mut_ptr(),
                operands_raw.len() as _,
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn nop(&self) -> Expr {
        let raw_expr = unsafe { ffi::BinaryenNop(self.inner.raw) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn unreachable(&self) -> Expr {
        let raw_expr = unsafe { ffi::BinaryenUnreachable(self.inner.raw) };
        Expr::from_raw(self, raw_expr)
    }

    pub fn select(&self, condition: Expr, if_true: Expr, if_false: Expr) -> Expr {
        let raw_expr = unsafe {
            ffi::BinaryenSelect(
                self.inner.raw,
                condition.into_raw(),
                if_true.into_raw(),
                if_false.into_raw(),
            )
        };
        Expr::from_raw(self, raw_expr)
    }

    pub fn drop(&self, value: Expr) -> Expr {
        let raw_expr = unsafe { ffi::BinaryenDrop(self.inner.raw, value.into_raw()) };
        Expr::from_raw(self, raw_expr)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HostOp {
    PageSize,
    CurrentMemory,
    GrowMemory,
    HasFeature,
}

impl From<HostOp> for ffi::BinaryenOp {
    fn from(hostop: HostOp) -> ffi::BinaryenOp {
        use HostOp::*;
        unsafe {
            match hostop {
                PageSize => ffi::BinaryenPageSize(),
                CurrentMemory => ffi::BinaryenCurrentMemory(),
                GrowMemory => ffi::BinaryenGrowMemory(),
                HasFeature => ffi::BinaryenHasFeature(),
            }
        }
    }
}

impl Default for Module {
    fn default() -> Module {
        Module::new()
    }
}

pub struct Segment<'a> {
    data: &'a [u8],
    offset_expr: Expr,
}

impl<'a> Segment<'a> {
    pub fn new(data: &[u8], offset_expr: Expr) -> Segment {
        Segment { data, offset_expr }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnaryOp {
    ClzI32,
    CtzI32,
    PopcntI32,
    NegF32,
    AbsF32,
    CeilF32,
    FloorF32,
    TruncF32,
    NearestF32,
    SqrtF32,
    EqZI32,
    ClzI64,
    CtzI64,
    PopcntI64,
    NegF64,
    AbsF64,
    CeilF64,
    FloorF64,
    TruncF64,
    NearestF64,
    SqrtF64,
    EqZI64,
    ExtendSI32,
    ExtendUI32,
    WrapI64,
    TruncSF32ToI32,
    TruncSF32ToI64,
    TruncUF32ToI32,
    TruncUF32ToI64,
    TruncSF64ToI32,
    TruncSF64ToI64,
    TruncUF64ToI32,
    TruncUF64ToI64,
    ReinterpretF32,
    ReinterpretF64,
    ConvertSI32ToF32,
    ConvertSI32ToF64,
    ConvertUI32ToF32,
    ConvertUI32ToF64,
    ConvertSI64ToF32,
    ConvertSI64ToF64,
    ConvertUI64ToF32,
    ConvertUI64ToF64,
    PromoteF32,
    DemoteF64,
    ReinterpretI32,
    ReinterpretI64,
}

impl From<UnaryOp> for ffi::BinaryenOp {
    fn from(unop: UnaryOp) -> ffi::BinaryenOp {
        use UnaryOp::*;
        unsafe {
            match unop {
                ClzI32 => ffi::BinaryenClzInt32(),
                CtzI32 => ffi::BinaryenCtzInt32(),
                PopcntI32 => ffi::BinaryenPopcntInt32(),
                NegF32 => ffi::BinaryenNegFloat32(),
                AbsF32 => ffi::BinaryenAbsFloat32(),
                CeilF32 => ffi::BinaryenCeilFloat32(),
                FloorF32 => ffi::BinaryenFloorFloat32(),
                TruncF32 => ffi::BinaryenTruncFloat32(),
                NearestF32 => ffi::BinaryenNearestFloat32(),
                SqrtF32 => ffi::BinaryenSqrtFloat32(),
                EqZI32 => ffi::BinaryenEqZInt32(),
                ClzI64 => ffi::BinaryenClzInt64(),
                CtzI64 => ffi::BinaryenCtzInt64(),
                PopcntI64 => ffi::BinaryenPopcntInt64(),
                NegF64 => ffi::BinaryenNegFloat64(),
                AbsF64 => ffi::BinaryenAbsFloat64(),
                CeilF64 => ffi::BinaryenCeilFloat64(),
                FloorF64 => ffi::BinaryenFloorFloat64(),
                TruncF64 => ffi::BinaryenTruncFloat64(),
                NearestF64 => ffi::BinaryenNearestFloat64(),
                SqrtF64 => ffi::BinaryenSqrtFloat64(),
                EqZI64 => ffi::BinaryenEqZInt64(),
                ExtendSI32 => ffi::BinaryenExtendSInt32(),
                ExtendUI32 => ffi::BinaryenExtendUInt32(),
                WrapI64 => ffi::BinaryenWrapInt64(),
                TruncSF32ToI32 => ffi::BinaryenTruncSFloat32ToInt32(),
                TruncSF32ToI64 => ffi::BinaryenTruncSFloat32ToInt64(),
                TruncUF32ToI32 => ffi::BinaryenTruncUFloat32ToInt32(),
                TruncUF32ToI64 => ffi::BinaryenTruncUFloat32ToInt64(),
                TruncSF64ToI32 => ffi::BinaryenTruncSFloat64ToInt32(),
                TruncSF64ToI64 => ffi::BinaryenTruncSFloat64ToInt64(),
                TruncUF64ToI32 => ffi::BinaryenTruncUFloat64ToInt32(),
                TruncUF64ToI64 => ffi::BinaryenTruncUFloat64ToInt64(),
                ReinterpretF32 => ffi::BinaryenReinterpretFloat32(),
                ReinterpretF64 => ffi::BinaryenReinterpretFloat64(),
                ConvertSI32ToF32 => ffi::BinaryenConvertSInt32ToFloat32(),
                ConvertSI32ToF64 => ffi::BinaryenConvertSInt32ToFloat64(),
                ConvertUI32ToF32 => ffi::BinaryenConvertUInt32ToFloat32(),
                ConvertUI32ToF64 => ffi::BinaryenConvertUInt32ToFloat64(),
                ConvertSI64ToF32 => ffi::BinaryenConvertSInt64ToFloat32(),
                ConvertSI64ToF64 => ffi::BinaryenConvertSInt64ToFloat64(),
                ConvertUI64ToF32 => ffi::BinaryenConvertUInt64ToFloat32(),
                ConvertUI64ToF64 => ffi::BinaryenConvertUInt64ToFloat64(),
                PromoteF32 => ffi::BinaryenPromoteFloat32(),
                DemoteF64 => ffi::BinaryenDemoteFloat64(),
                ReinterpretI32 => ffi::BinaryenReinterpretInt32(),
                ReinterpretI64 => ffi::BinaryenReinterpretInt64(),
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    AddI32,
    SubI32,
    MulI32,
    DivSI32,
    DivUI32,
    RemSI32,
    RemUI32,
    AndI32,
    OrI32,
    XorI32,
    ShlI32,
    ShrUI32,
    ShrSI32,
    RotLI32,
    RotRI32,
    EqI32,
    NeI32,
    LtSI32,
    LtUI32,
    LeSI32,
    LeUI32,
    GtSI32,
    GtUI32,
    GeSI32,
    GeUI32,
    AddI64,
    SubI64,
    MulI64,
    DivSI64,
    DivUI64,
    RemSI64,
    RemUI64,
    AndI64,
    OrI64,
    XorI64,
    ShlI64,
    ShrUI64,
    ShrSI64,
    RotLI64,
    RotRI64,
    EqI64,
    NeI64,
    LtSI64,
    LtUI64,
    LeSI64,
    LeUI64,
    GtSI64,
    GtUI64,
    GeSI64,
    GeUI64,
    AddF32,
    SubF32,
    MulF32,
    DivF32,
    CopySignF32,
    MinF32,
    MaxF32,
    EqF32,
    NeF32,
    LtF32,
    LeF32,
    GtF32,
    GeF32,
    AddF64,
    SubF64,
    MulF64,
    DivF64,
    CopySignF64,
    MinF64,
    MaxF64,
    EqF64,
    NeF64,
    LtF64,
    LeF64,
    GtF64,
    GeF64,
}

impl From<BinaryOp> for ffi::BinaryenOp {
    fn from(binop: BinaryOp) -> ffi::BinaryenOp {
        use BinaryOp::*;
        unsafe {
            match binop {
                AddI32 => ffi::BinaryenAddInt32(),
                SubI32 => ffi::BinaryenSubInt32(),
                MulI32 => ffi::BinaryenMulInt32(),
                DivSI32 => ffi::BinaryenDivSInt32(),
                DivUI32 => ffi::BinaryenDivUInt32(),
                RemSI32 => ffi::BinaryenRemSInt32(),
                RemUI32 => ffi::BinaryenRemUInt32(),
                AndI32 => ffi::BinaryenAndInt32(),
                OrI32 => ffi::BinaryenOrInt32(),
                XorI32 => ffi::BinaryenXorInt32(),
                ShlI32 => ffi::BinaryenShlInt32(),
                ShrUI32 => ffi::BinaryenShrUInt32(),
                ShrSI32 => ffi::BinaryenShrSInt32(),
                RotLI32 => ffi::BinaryenRotLInt32(),
                RotRI32 => ffi::BinaryenRotRInt32(),
                EqI32 => ffi::BinaryenEqInt32(),
                NeI32 => ffi::BinaryenNeInt32(),
                LtSI32 => ffi::BinaryenLtSInt32(),
                LtUI32 => ffi::BinaryenLtUInt32(),
                LeSI32 => ffi::BinaryenLeSInt32(),
                LeUI32 => ffi::BinaryenLeUInt32(),
                GtSI32 => ffi::BinaryenGtSInt32(),
                GtUI32 => ffi::BinaryenGtUInt32(),
                GeSI32 => ffi::BinaryenGeSInt32(),
                GeUI32 => ffi::BinaryenGeUInt32(),
                AddI64 => ffi::BinaryenAddInt64(),
                SubI64 => ffi::BinaryenSubInt64(),
                MulI64 => ffi::BinaryenMulInt64(),
                DivSI64 => ffi::BinaryenDivSInt64(),
                DivUI64 => ffi::BinaryenDivUInt64(),
                RemSI64 => ffi::BinaryenRemSInt64(),
                RemUI64 => ffi::BinaryenRemUInt64(),
                AndI64 => ffi::BinaryenAndInt64(),
                OrI64 => ffi::BinaryenOrInt64(),
                XorI64 => ffi::BinaryenXorInt64(),
                ShlI64 => ffi::BinaryenShlInt64(),
                ShrUI64 => ffi::BinaryenShrUInt64(),
                ShrSI64 => ffi::BinaryenShrSInt64(),
                RotLI64 => ffi::BinaryenRotLInt64(),
                RotRI64 => ffi::BinaryenRotRInt64(),
                EqI64 => ffi::BinaryenEqInt64(),
                NeI64 => ffi::BinaryenNeInt64(),
                LtSI64 => ffi::BinaryenLtSInt64(),
                LtUI64 => ffi::BinaryenLtUInt64(),
                LeSI64 => ffi::BinaryenLeSInt64(),
                LeUI64 => ffi::BinaryenLeUInt64(),
                GtSI64 => ffi::BinaryenGtSInt64(),
                GtUI64 => ffi::BinaryenGtUInt64(),
                GeSI64 => ffi::BinaryenGeSInt64(),
                GeUI64 => ffi::BinaryenGeUInt64(),
                AddF32 => ffi::BinaryenAddFloat32(),
                SubF32 => ffi::BinaryenSubFloat32(),
                MulF32 => ffi::BinaryenMulFloat32(),
                DivF32 => ffi::BinaryenDivFloat32(),
                CopySignF32 => ffi::BinaryenCopySignFloat32(),
                MinF32 => ffi::BinaryenMinFloat32(),
                MaxF32 => ffi::BinaryenMaxFloat32(),
                EqF32 => ffi::BinaryenEqFloat32(),
                NeF32 => ffi::BinaryenNeFloat32(),
                LtF32 => ffi::BinaryenLtFloat32(),
                LeF32 => ffi::BinaryenLeFloat32(),
                GtF32 => ffi::BinaryenGtFloat32(),
                GeF32 => ffi::BinaryenGeFloat32(),
                AddF64 => ffi::BinaryenAddFloat64(),
                SubF64 => ffi::BinaryenSubFloat64(),
                MulF64 => ffi::BinaryenMulFloat64(),
                DivF64 => ffi::BinaryenDivFloat64(),
                CopySignF64 => ffi::BinaryenCopySignFloat64(),
                MinF64 => ffi::BinaryenMinFloat64(),
                MaxF64 => ffi::BinaryenMaxFloat64(),
                EqF64 => ffi::BinaryenEqFloat64(),
                NeF64 => ffi::BinaryenNeFloat64(),
                LtF64 => ffi::BinaryenLtFloat64(),
                LeF64 => ffi::BinaryenLeFloat64(),
                GtF64 => ffi::BinaryenGtFloat64(),
                GeF64 => ffi::BinaryenGeFloat64(),
            }
        }
    }
}

pub struct FnType {
    raw: ffi::BinaryenFunctionTypeRef,
}

pub struct FnRef {
    inner: ffi::BinaryenFunctionRef,
}

/// Type of the values. For example, these can be found on a stack and
/// in local vars.
#[derive(Copy, Clone)]
pub enum ValueTy {
    I32,
    I64,
    F32,
    F64,
}

pub struct Ty(Option<ValueTy>);

impl Ty {
    pub fn none() -> Ty {
        Ty(None)
    }

    pub fn value(ty: ValueTy) -> Ty {
        Ty(Some(ty))
    }
}

impl From<ValueTy> for ffi::BinaryenType {
    fn from(ty: ValueTy) -> ffi::BinaryenType {
        unsafe {
            match ty {
                ValueTy::I32 => ffi::BinaryenInt32(),
                ValueTy::I64 => ffi::BinaryenInt64(),
                ValueTy::F32 => ffi::BinaryenFloat32(),
                ValueTy::F64 => ffi::BinaryenFloat64(),
            }
        }
    }
}

impl From<Ty> for ffi::BinaryenType {
    fn from(ty: Ty) -> ffi::BinaryenType {
        match ty.0 {
            Some(ty) => ty.into(),
            None => unsafe { ffi::BinaryenNone() },
        }
    }
}

#[derive(Clone)]
pub struct Expr {
    _module_ref: Rc<InnerModule>,
    raw: ffi::BinaryenExpressionRef,
}

impl Expr {
    pub fn from_raw(module: &Module, raw: ffi::BinaryenExpressionRef) -> Expr {
        Expr {
            _module_ref: Rc::clone(&module.inner),
            raw,
        }
    }

    pub unsafe fn into_raw(self) -> ffi::BinaryenExpressionRef {
        self.raw
    }

    pub fn print(&self) {
        unsafe { ffi::BinaryenExpressionPrint(self.raw) }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Literal {
    I32(u32),
    I64(u64),
    F32(f32),
    F64(f64),
}

impl From<Literal> for ffi::BinaryenLiteral {
    fn from(literal: Literal) -> ffi::BinaryenLiteral {
        unsafe {
            match literal {
                Literal::I32(v) => ffi::BinaryenLiteralInt32(v as i32),
                Literal::I64(v) => ffi::BinaryenLiteralInt64(v as i64),
                Literal::F32(v) => ffi::BinaryenLiteralFloat32(v),
                Literal::F64(v) => ffi::BinaryenLiteralFloat64(v),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // see https://github.com/WebAssembly/binaryen/blob/master/test/example/c-api-hello-world.c
    #[test]
    fn test_hello_world() {
        let module = Module::new();

        let params = &[ValueTy::I32, ValueTy::I32];
        let iii = module.add_fn_type(Some("iii"), params, Ty::value(ValueTy::I32));

        let x = module.get_local(0, ValueTy::I32);
        let y = module.get_local(1, ValueTy::I32);
        let add = module.binary(BinaryOp::AddI32, x, y);

        let _adder = module.add_fn("adder", &iii, &[], add);

        assert!(module.is_valid());
    }

    #[test]
    fn test_simple() {
        let module = Module::new();

        let main_fn_ty = module.add_fn_type(Some("main_fn_ty"), &[], Ty::none());

        {
            let segment_data = b"Hello world\0";
            let segment_offset_expr = module.const_(Literal::I32(0));
            let segments = vec![Segment::new(segment_data, segment_offset_expr)];
            module.set_memory(1, 1, Some("mem"), segments);
        }

        let nop = module.nop();
        let main = module.add_fn("main", &main_fn_ty, &[], nop);
        module.set_start(&main);

        assert!(module.is_valid());

        let written_wasm = module.write();
        let read_wasm = Module::read(&written_wasm);
        assert!(read_wasm.is_valid());
    }

    #[should_panic]
    #[test]
    fn test_relooper_with_different_module() {
        let module1 = Module::new();
        let mut relooper = module1.relooper();

        let module2 = Module::new();
        // Should panic here.
        relooper.add_block(module2.nop());
    }

    #[test]
    fn test_use_same_expr_twice() {
        let module = Module::new();
        let expr = module.nop();
        let expr_copy = Expr::from_raw(&module, expr.raw);

        module.block(None::<&str>, vec![expr, expr_copy], Ty::none());
    }

    #[test]
    fn test_unreachable() {
        let module = Module::new();

        let params = &[];
        let return_i32 = module.add_fn_type(None::<&str>, params, Ty::value(ValueTy::I32));
        let _ = module.add_fn_type(Some("return_i64"), params, Ty::value(ValueTy::I64));

        let unreachable = module.unreachable();

        let add = module.call_indirect(unreachable, vec![], "return_i64");

        let _test = module.add_fn("test", &return_i32, &[], add);

        assert!(module.is_valid());
    }
}
