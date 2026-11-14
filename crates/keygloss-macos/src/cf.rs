//! Minimal CoreFoundation, CoreGraphics, Accessibility, Text Input Sources and libdispatch FFI.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_void;
use std::ptr;

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFArrayRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFDataRef = *const c_void;
pub type CFRunLoopRef = *const c_void;
pub type CFRunLoopSourceRef = *const c_void;
pub type CFRunLoopTimerRef = *const c_void;
pub type CFMachPortRef = *const c_void;
pub type CFAllocatorRef = *const c_void;
pub type CFIndex = isize;
pub type CGEventRef = *const c_void;
pub type CGEventSourceRef = *const c_void;
pub type CGEventTapProxy = *const c_void;
pub type AXUIElementRef = *const c_void;
pub type TISInputSourceRef = *const c_void;

pub const kCFStringEncodingUTF8: u32 = 0x0800_0100;

#[repr(C)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

#[repr(C)]
pub struct CFRunLoopTimerContext {
    pub version: CFIndex,
    pub info: *mut c_void,
    pub retain: *const c_void,
    pub release: *const c_void,
    pub copy_description: *const c_void,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub static kCFBooleanTrue: CFTypeRef;
    pub static kCFTypeDictionaryKeyCallBacks: c_void;
    pub static kCFTypeDictionaryValueCallBacks: c_void;

    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFGetTypeID(cf: CFTypeRef) -> usize;
    pub fn CFStringGetTypeID() -> usize;
    pub fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        len: CFIndex,
        encoding: u32,
        external: u8,
    ) -> CFStringRef;
    pub fn CFStringGetLength(s: CFStringRef) -> CFIndex;
    pub fn CFStringGetCharacters(s: CFStringRef, range: CFRange, buffer: *mut u16);
    pub fn CFArrayGetCount(a: CFArrayRef) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(a: CFArrayRef, i: CFIndex) -> *const c_void;
    pub fn CFDictionaryCreate(
        alloc: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFDictionaryRef;
    pub fn CFDataGetBytePtr(d: CFDataRef) -> *const u8;

    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopRun();
    pub fn CFRunLoopStop(rl: CFRunLoopRef);
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopAddTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: CFStringRef);
    pub fn CFRunLoopTimerCreate(
        alloc: CFAllocatorRef,
        fire_date: f64,
        interval: f64,
        flags: u64,
        order: CFIndex,
        callout: extern "C" fn(CFRunLoopTimerRef, *mut c_void),
        context: *mut CFRunLoopTimerContext,
    ) -> CFRunLoopTimerRef;
    pub fn CFRunLoopTimerInvalidate(timer: CFRunLoopTimerRef);
    pub fn CFAbsoluteTimeGetCurrent() -> f64;
    pub fn CFMachPortCreateRunLoopSource(alloc: CFAllocatorRef, port: CFMachPortRef, order: CFIndex) -> CFRunLoopSourceRef;
    pub fn CFMachPortInvalidate(port: CFMachPortRef);

    pub fn CFNotificationCenterGetDistributedCenter() -> *const c_void;
    pub fn CFNotificationCenterAddObserver(
        center: *const c_void,
        observer: *const c_void,
        callback: extern "C" fn(*const c_void, *mut c_void, CFStringRef, *const c_void, CFDictionaryRef),
        name: CFStringRef,
        object: *const c_void,
        suspension: CFIndex,
    );
}

pub const CFNotificationSuspensionBehaviorDeliverImmediately: CFIndex = 4;

// CGEventType
pub const kCGEventLeftMouseDown: u32 = 1;
pub const kCGEventRightMouseDown: u32 = 3;
pub const kCGEventKeyDown: u32 = 10;
pub const kCGEventKeyUp: u32 = 11;
pub const kCGEventFlagsChanged: u32 = 12;
pub const kCGEventOtherMouseDown: u32 = 25;
pub const kCGEventTapDisabledByTimeout: u32 = 0xFFFF_FFFE;
pub const kCGEventTapDisabledByUserInput: u32 = 0xFFFF_FFFF;

// CGEventField
pub const kCGKeyboardEventAutorepeat: u32 = 8;
pub const kCGKeyboardEventKeycode: u32 = 9;
pub const kCGEventSourceUserData: u32 = 42;

// CGEventFlags
pub const kCGEventFlagMaskAlphaShift: u64 = 0x0001_0000;
pub const kCGEventFlagMaskShift: u64 = 0x0002_0000;
pub const kCGEventFlagMaskControl: u64 = 0x0004_0000;
pub const kCGEventFlagMaskAlternate: u64 = 0x0008_0000;
pub const kCGEventFlagMaskCommand: u64 = 0x0010_0000;
pub const kCGEventFlagMaskSecondaryFn: u64 = 0x0080_0000;

// CGEventTapLocation, placement, options; CGEventSourceStateID
pub const kCGHIDEventTap: u32 = 0;
pub const kCGSessionEventTap: u32 = 1;
pub const kCGHeadInsertEventTap: u32 = 0;
pub const kCGEventTapOptionDefault: u32 = 0;
pub const kCGEventSourceStatePrivate: i32 = -1;

pub type CGEventTapCallBack = extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    pub fn CGEventSetFlags(event: CGEventRef, flags: u64);
    pub fn CGEventCreateCopy(event: CGEventRef) -> CGEventRef;
    pub fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, down: bool) -> CGEventRef;
    pub fn CGEventKeyboardSetUnicodeString(event: CGEventRef, len: usize, chars: *const u16);
    pub fn CGEventPost(tap: u32, event: CGEventRef);
    pub fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(element: AXUIElementRef, attribute: CFStringRef, value: *mut CFTypeRef) -> i32;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> i32;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub static kTISPropertyInputSourceCategory: CFStringRef;
    pub static kTISPropertyInputSourceType: CFStringRef;
    pub static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    pub static kTISPropertyInputSourceID: CFStringRef;
    pub static kTISPropertyLocalizedName: CFStringRef;
    pub static kTISPropertyInputSourceLanguages: CFStringRef;
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    pub static kTISCategoryKeyboardInputSource: CFStringRef;
    pub static kTISTypeKeyboardLayout: CFStringRef;
    pub static kTISNotifySelectedKeyboardInputSourceChanged: CFStringRef;
    pub static kTISNotifyEnabledKeyboardInputSourcesChanged: CFStringRef;

    pub fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    pub fn TISCreateInputSourceList(properties: CFDictionaryRef, include_all_installed: bool) -> CFArrayRef;
    pub fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    pub fn TISSelectInputSource(source: TISInputSourceRef) -> i32;
    pub fn LMGetKbdType() -> u8;
    pub fn IsSecureEventInputEnabled() -> bool;
    #[allow(clippy::too_many_arguments)]
    pub fn UCKeyTranslate(
        layout: *const u8,
        keycode: u16,
        action: u16,
        modifiers: u32,
        keyboard_type: u32,
        options: u32,
        dead_key_state: *mut u32,
        max_len: usize,
        actual_len: *mut usize,
        chars: *mut u16,
    ) -> i32;
}

unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn pthread_main_np() -> i32;
}

/// True on the process's main thread.
pub fn is_main_thread() -> bool {
    unsafe { pthread_main_np() != 0 }
}

/// Runs `f` on the main thread without waiting (inline if already there).
pub fn on_main(f: impl FnOnce() + Send + 'static) {
    if is_main_thread() {
        f();
        return;
    }
    extern "C" fn trampoline(ctx: *mut c_void) {
        // SAFETY: `ctx` is the box leaked below, consumed exactly once.
        let f = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce()>) };
        f();
    }
    let boxed: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(boxed).cast(), trampoline) };
}

/// An owned (+1) CoreFoundation reference, released on drop.
pub struct Owned(pub CFTypeRef);

impl Owned {
    /// Takes ownership of a +1 reference; `None` for null.
    pub fn new(r: CFTypeRef) -> Option<Owned> {
        (!r.is_null()).then_some(Owned(r))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

// CF objects are immutable here and CoreFoundation reference counting is thread-safe.
unsafe impl Send for Owned {}
unsafe impl Sync for Owned {}

/// A new CFString (+1).
pub fn cfstr(s: &str) -> Owned {
    let r = unsafe { CFStringCreateWithBytes(ptr::null(), s.as_ptr(), s.len() as CFIndex, kCFStringEncodingUTF8, 0) };
    Owned(r)
}

/// Copies a CFString; `None` for null or a non-string.
pub fn string(s: CFTypeRef) -> Option<String> {
    if s.is_null() || unsafe { CFGetTypeID(s) != CFStringGetTypeID() } {
        return None;
    }
    let len = unsafe { CFStringGetLength(s) };
    let mut buf = vec![0u16; len as usize];
    unsafe { CFStringGetCharacters(s, CFRange { location: 0, length: len }, buf.as_mut_ptr()) };
    Some(String::from_utf16_lossy(&buf))
}

/// The elements of a CFArray (not retained).
pub fn array(a: CFArrayRef) -> Vec<*const c_void> {
    if a.is_null() {
        return Vec::new();
    }
    (0..unsafe { CFArrayGetCount(a) }).map(|i| unsafe { CFArrayGetValueAtIndex(a, i) }).collect()
}

/// A CFDictionary (+1) of CF keys and values.
pub fn dict(pairs: &[(CFTypeRef, CFTypeRef)]) -> Owned {
    let keys: Vec<_> = pairs.iter().map(|p| p.0).collect();
    let values: Vec<_> = pairs.iter().map(|p| p.1).collect();
    Owned(unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            pairs.len() as CFIndex,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        )
    })
}
