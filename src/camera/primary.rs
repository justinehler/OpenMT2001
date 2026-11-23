//! Primary camera record arithmetic and an explicit mode12 callback.
//! Owners supply mode dispatch, scheduling, live input and trigonometry.
//! Common processing follows the mode callback; sampling runs at render.
use crate::x87::Sum;
use std::cmp::Ordering;

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primary { raw: [u8; 196] }

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub mode: i32,
    pub current: [[f32; 3]; 2],
    pub desired: [[f32; 3]; 2],
    pub sampled: [[f32; 3]; 2],
    pub up: [f32; 3],
    pub dirty: [u8; 3],
    pub divisor: i32,
    pub threshold: f32,
    pub yaw: f32,
    pub final_yaw: f32,
    pub sine: f32,
    pub cosine: f32,
    pub parameter: u32,
    pub radius: f32,
    pub center: [f32; 3],
}

/// Explicit external obligation: native FSIN/FCOS precision is not supplied by
/// Sum. Inputs retain PC64 values, including the unrounded FST yaw for sine.
/// Implementations must document their accuracy; an arbitrary provider does
/// not make this core an exact original transcendental implementation.
pub trait AngularTrig {
    fn sine(&mut self, retained_yaw: Sum) -> Result<Sum>;
    fn cosine(&mut self, rounded_yaw: Sum) -> Result<Sum>;
}
struct MissingTrig;
impl AngularTrig for MissingTrig {
    fn sine(&mut self, _: Sum) -> Result<Sum> { Err("native FSIN provider required") }
    fn cosine(&mut self, _: Sum) -> Result<Sum> { Err("native FCOS provider required") }
}

impl Primary {
    /// Supported finite mode6/12 subset. Opaque bytes are never decoded as floats.
    /// Positive divisors are required whenever a dirty flag can cause sampling.
    pub fn decode(raw: [u8; 196]) -> Result<Self> {
        let state = Self { raw };
        state.validate()?;
        Ok(state)
    }
    pub fn raw(&self) -> &[u8; 196] { &self.raw }
    fn word(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.raw[at..at+4].try_into().unwrap())
    }
    fn f(&self, at: usize) -> f32 { f32::from_bits(self.word(at)) }
    fn triple(&self, at: usize) -> [f32; 3] {
        std::array::from_fn(|i| self.f(at + i*4))
    }
    pub fn view(&self) -> View {
        View {
            mode: self.word(0) as i32,
            current: [self.triple(4), self.triple(0x10)],
            desired: [self.triple(0x34), self.triple(0x40)],
            sampled: [self.triple(0x4c), self.triple(0x58)],
            up: self.triple(0x64), dirty: self.raw[0x70..0x73].try_into().unwrap(),
            divisor: self.word(0x74) as i32, threshold: self.f(0x78),
            yaw: self.f(0x7c), final_yaw: self.f(0x98),
            sine: self.f(0x88), cosine: self.f(0x90),
            parameter: self.word(0xa8), radius: self.f(0xb0), center: self.triple(0xb4),
        }
    }
    fn validate(&self) -> Result<()> {
        if !matches!(self.word(0) as i32, 6 | 12) {
            return Err("primary core supports only mode6/12");
        }
        for at in (4..0x1c).step_by(4).chain((0x34..0x70).step_by(4))
            .chain([0x78,0x7c,0x88,0x90,0x98,0xb0,0xb4,0xb8,0xbc]) {
            if !self.f(at).is_finite() { return Err("nonfinite consumed primary field"); }
        }
        if self.raw[0x70..0x73].iter().any(|&b| b != 0) && self.word(0x74) as i32 <= 0 {
            return Err("dirty primary requires supported positive signed divisor");
        }
        Ok(())
    }
    fn store(&mut self, at: usize, value: Sum) -> Result<()> {
        let value = value.to_f32();
        if !value.is_finite() { return Err("primary float32 store overflow"); }
        self.raw[at..at+4].copy_from_slice(&value.to_bits().to_le_bytes());
        Ok(())
    }
    fn ext(&self, at: usize) -> Result<Sum> { Sum::from_f32(self.f(at)) }

    /// Exact-only default: active angular smoothing without a proved native
    /// transcendental provider is explicitly unsupported. Errors are atomic.
    pub fn common(&mut self) -> Result<()> { self.common_with_trig(&mut MissingTrig) }
    pub fn common_with_trig(&mut self, trig: &mut impl AngularTrig) -> Result<()> {
        let mut next = self.clone();
        next.common_inner(trig)?;
        next.validate()?;
        *self = next;
        Ok(())
    }
    fn common_inner(&mut self, trig: &mut impl AngularTrig) -> Result<()> {
        if self.word(0xa8) == 2 {
            let mut z = self.ext(0x48)?;
            z.add(Sum::from_f64(0.1)?.negated());
            // MOV transport is bit preserving, including negative zero.
            self.raw.copy_within(0x40..0x44, 0x34);
            self.store(0x3c, z)?;
            let mut floor = self.ext(0x44)?;
            floor.add_f32(150.)?;
            if self.ext(0x38)?.compare(floor) == Ordering::Less {
                self.store(0x38, floor)?;
            }
        } else if self.raw[0x72] != 0 {
            let mut yaw = self.ext(0x98)?;
            yaw.add(self.ext(0x7c)?.negated());
            yaw.div(Sum::from_i32(self.word(0x74) as i32))?;
            yaw.add(self.ext(0x7c)?);
            self.store(0x7c, yaw)?; // FST: retain yaw for FSIN, not the rounded field.
            self.store(0x88, trig.sine(yaw)?)?;
            self.store(0x90, trig.cosine(self.ext(0x7c)?)?)?;
            for (cache, center, dest) in [(0x90,0xb4,0x34),(0x88,0xbc,0x3c)] {
                let mut eye = self.ext(0xb0)?;
                eye.mul(self.ext(cache)?);
                eye.add(self.ext(center)?);
                self.store(dest, eye)?;
            }
        }
        let ticks = self.word(0x74) as i32; // shared predicate capture
        for (flag, current, desired) in [(0x70,4,0x34),(0x71,0x10,0x40)] {
            let mut smooth = false;
            if self.raw[flag] != 0 && ticks > 1 {
                let threshold = self.ext(0x78)?;
                if threshold.compare(Sum::default()) == Ordering::Equal {
                    smooth = true;
                } else {
                    let mut distance = Sum::default();
                    for i in 0..3 {
                        let mut delta = self.ext(desired+i*4)?;
                        delta.add(self.ext(current+i*4)?.negated());
                        delta = match delta.compare(Sum::default()) {
                            Ordering::Less => delta.negated(),
                            Ordering::Equal => Sum::default(), // FABS clears -0
                            Ordering::Greater => delta,
                        };
                        distance.add(delta);
                    }
                    smooth = distance.compare(threshold) == Ordering::Less;
                }
            }
            if smooth {
                let divisor = Sum::from_i32(self.word(0x74) as i32);
                for i in 0..3 {
                    let mut value = self.ext(desired+i*4)?;
                    value.add(self.ext(current+i*4)?.negated());
                    value.div(divisor)?;
                    value.add(self.ext(current+i*4)?);
                    self.store(current+i*4, value)?;
                }
            } else {
                self.raw.copy_within(desired..desired+12, current);
            }
        }
        Ok(()) // neither countdown nor dirty flag clearing
    }
    /// Native next-step fraction, not previous/current lerp. Supports finite
    /// normal residual fractions only; phase ownership is outside this core.
    pub fn sample(&mut self, amount: f32) -> Result<[[f32; 3]; 2]> {
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("unsupported camera residual fraction");
        }
        let mut next = self.clone();
        for (flag, current, desired, sampled) in [(0x70,4,0x34,0x4c),(0x71,0x10,0x40,0x58)] {
            if next.raw[flag] != 0 {
                let divisor = Sum::from_i32(next.word(0x74) as i32);
                for i in 0..3 {
                    let mut value = next.ext(desired+i*4)?;
                    value.add(next.ext(current+i*4)?.negated());
                    value.mul_f32(amount)?;
                    value.div(divisor)?;
                    value.add(next.ext(current+i*4)?);
                    next.store(sampled+i*4, value)?;
                }
            } else { next.raw.copy_within(current..current+12, sampled); }
        }
        next.validate()?;
        let sampled = next.view().sampled;
        *self = next;
        Ok(sampled)
    }
}

/// Captured source reads of the SAME native920889 byte at ordered phases.
/// Different values describe sampled changes, not independent gameplay policies.
#[derive(Clone, Copy, Debug)]
pub struct Mode12Input {
    pub build_visible: bool,
    pub enable_translation: bool,
    pub enable_keyboard: bool,
    pub enable_rotation: bool,
    pub enable_zoom: bool,
    pub buttons: [bool; 2], // left, right (nonzero native bytes)
    pub delta: [i32; 2],
    pub keys: [bool; 4], // Right, Left, Down, Up
    pub sensitivity: i32,
    pub legacy_yaw: bool,
    pub invert_vertical: bool,
    pub low_height: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorRequest { pub state: u32, pub priority: i32 }

impl Primary {
    /// Callback only: caller dispatches actual mode12, then runs common().
    /// Pure captured input, explicit trig dependency, ordered deferred effects.
    /// Primary/errors are atomic; external provider side effects are not undone.
    pub fn mode12(&mut self, input: Mode12Input, trig: &mut impl AngularTrig)
        -> Result<Vec<CursorRequest>> {
        if self.view().mode != 12 { return Err("mode12 callback requires mode12"); }
        let mut next = self.clone();
        let effects = next.mode12_inner(input, trig)?;
        next.validate()?;
        *self = next;
        Ok(effects)
    }
    fn mode12_inner(&mut self, i: Mode12Input, trig: &mut impl AngularTrig)
        -> Result<Vec<CursorRequest>> {
        let mut effects = Vec::new();
        let mut tx = self.ext(0x88)?;
        tx.mul_f32(25.)?;
        tx.add(self.ext(0xb4)?);
        let mut cz = self.ext(0x90)?;
        cz.mul_f32(25.)?;
        let mut tz = self.ext(0xbc)?;
        tz.add(cz.negated());
        let tz = tz.to_f32(); // native scratch spill before cosine
        if !tz.is_finite() { return Err("mode12 target Z overflow"); }
        self.raw.copy_within(0xb8..0xbc, 0x44);
        let mut eye = trig.cosine(self.ext(0x7c)?)?;
        self.raw[0x48..0x4c].copy_from_slice(&tz.to_le_bytes());
        eye.mul(self.ext(0xb0)?);
        eye.add(tx);
        self.store(0x34, eye)?;
        let mut eye = trig.sine(self.ext(0x7c)?)?;
        eye.mul(self.ext(0xb0)?);
        eye.add(Sum::from_f32(tz)?);
        self.store(0x3c, eye)?;
        self.store(0x40, tx)?;

        let both = i.buttons == [true, true];
        let right_only = i.buttons == [false, true];
        // Native disabled checks inside the translation path jump directly to
        // zoom, whereas Build's visibility jump still reaches rotation.
        let mut rotation_reachable = true;
        if !i.build_visible {
            if !i.enable_translation { rotation_reachable = false; }
            else {
                let mut speed = Sum::from_i32(i.sensitivity);
                speed.mul_f32(0.01)?;
                speed.add(speed);
                speed.add_f32(1.)?;
                let step = speed.to_f32();
                if !step.is_finite() { return Err("mode12 pan step overflow"); }
                if both {
                    effects.push(CursorRequest { state: 3, priority: 1 });
                    for (delta, xcache, zcache, xsub, zsub) in [
                        (i.delta[0],0x88,0x90,i.delta[0]>0,i.delta[0]<0),
                        (i.delta[1],0x90,0x88,i.delta[1]<0,i.delta[1]<0),
                    ] {
                        if delta != 0 {
                            let magnitude = delta.checked_abs().ok_or("unsupported INT_MIN pan magnitude")?;
                            for (at, cache, subtract) in [(0xb4,xcache,xsub),(0xbc,zcache,zsub)] {
                                let mut displacement = Sum::from_i32(magnitude);
                                displacement.mul(self.ext(cache)?);
                                displacement.mul_f32(step)?;
                                let mut value = self.ext(at)?;
                                value.add(if subtract { displacement.negated() } else { displacement });
                                self.store(at, value)?;
                            }
                        }
                    }
                }
                if !i.enable_keyboard { rotation_reachable = false; }
                else {
                    for (active, xcache, zcache, xsub, zsub) in [
                        (i.keys[0],0x88,0x90,true,false),
                        (i.keys[1],0x88,0x90,false,true),
                        (i.keys[2],0x90,0x88,false,false),
                        (i.keys[3],0x90,0x88,true,true),
                    ] {
                        if active {
                            for (at,cache,subtract) in [(0xb4,xcache,xsub),(0xbc,zcache,zsub)] {
                                let mut displacement = self.ext(cache)?;
                                displacement.add(displacement);
                                let mut value = self.ext(at)?;
                                value.add(if subtract { displacement.negated() } else { displacement });
                                self.store(at,value)?;
                            }
                        }
                    }
                    for at in [0xb4,0xbc] {
                        let value = self.ext(at)?;
                        if value.compare(Sum::from_f32(-500.)?) == Ordering::Less {
                            self.store(at,Sum::from_f32(-500.)?)?;
                        } else if value.compare(Sum::from_f32(500.)?) == Ordering::Greater {
                            self.store(at,Sum::from_f32(500.)?)?;
                        }
                    }
                }
            }
        }
        if rotation_reachable && i.enable_rotation && self.word(0xa8)!=2 && right_only {
            effects.push(CursorRequest { state: 2, priority: 1 });
            if i.delta[0]!=0 {
                let numerator = i.delta[0].clamp(-14,14)*2;
                let mut yaw = self.ext(0x7c)?;
                if i.legacy_yaw {
                    let denominator = i.sensitivity.wrapping_add(50);
                    let quotient = numerator.checked_div(denominator).ok_or("unsupported mode12 IDIV")?;
                    yaw.add(Sum::from_i32(quotient));
                } else {
                    let denominator = 200i32.wrapping_sub(i.sensitivity);
                    let mut quotient = Sum::from_i32(numerator);
                    quotient.div(Sum::from_i32(denominator))?;
                    yaw.add(quotient.negated());
                }
                self.store(0x7c,yaw)?;
                let sine = trig.sine(self.ext(0x7c)?)?;
                self.raw.copy_within(0x7c..0x80,0x98);
                self.store(0x88,sine)?;
                let cosine = trig.cosine(self.ext(0x7c)?)?;
                self.store(0x90,cosine)?;
            }
        }
        if !i.enable_zoom { return Ok(effects); }
        if self.word(0xa8)==2 {
            if !right_only { return Ok(effects); }
            effects.push(CursorRequest { state: 2, priority: 1 });
            let dy = i.delta[1];
            if dy!=0 {
                let amount = Sum::from_i32(dy.checked_abs().ok_or("unsupported INT_MIN special zoom")?);
                let mut height = self.ext(0x38)?;
                height.add(if dy>0 { amount.negated() } else { amount });
                self.store(0x38,height)?;
            }
            return Ok(effects);
        }
        let mut step = Sum::from_i32(i.sensitivity);
        step.mul_f32(0.01)?;
        step.add(Sum::from_f64(0.5)?);
        let step = step.to_f32();
        if !step.is_finite() { return Err("mode12 zoom step overflow"); }
        if !right_only { return Ok(effects); }
        effects.push(CursorRequest { state: 2, priority: 1 });
        if i.delta[1]==0 { return Ok(effects); }
        let dy = if i.invert_vertical {
            i.delta[1].checked_neg().ok_or("unsupported INT_MIN inverted zoom")?
        } else { i.delta[1] };
        let amount = Sum::from_i32(dy.checked_abs().ok_or("unsupported INT_MIN zoom magnitude")?);
        let mut displacement = amount;
        displacement.mul_f32(step)?;
        let positive = dy>0;
        let mut gate = if positive { Sum::from_i32(if i.low_height {17} else {25}) }
            else { Sum::from_f32(200.)? };
        gate.add(if positive { displacement } else { displacement.negated() });
        let admitted = gate.compare(self.ext(0x38)?) == if positive { Ordering::Less } else { Ordering::Greater };
        if !admitted { return Ok(effects); }
        let mut height = self.ext(0x38)?;
        height.add(if positive {displacement.negated()} else {displacement});
        self.store(0x38,height)?;
        let mut half = displacement;
        half.mul_f32(0.5)?;
        let mut target = self.ext(0x44)?;
        target.add(if positive {half.negated()} else {half});
        self.store(0x44,target)?;
        let mut radial = amount;
        radial.mul_f32(if positive {-0.1} else {0.1})?;
        let mut radius = self.ext(0xb0)?;
        radius.add(radial.negated());
        self.store(0xb0,radius)?;
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(raw: &mut [u8;196], at: usize, value: f32) {
        raw[at..at+4].copy_from_slice(&value.to_bits().to_le_bytes());
    }
    fn fixture() -> [u8;196] {
        let mut r = [0;196]; r[0]=6; r[0x70]=1; r[0x71]=1; r[0x74]=4;
        put(&mut r,0x68,1.); put(&mut r,0x34,8.); put(&mut r,0x40,16.);
        r
    }
    #[test]
    fn fixed_divisor_next_step_and_independent_flags() {
        let mut p=Primary::decode(fixture()).unwrap();
        p.common().unwrap(); assert_eq!(p.view().current[0][0],2.);
        assert_eq!(p.view().current[1][0],4.);
        assert_eq!(p.sample(0.5).unwrap()[0][0],2.75);
        assert_eq!(p.view().current[0][0],2.);
        p.common().unwrap(); assert_eq!(p.view().current[0][0],3.5);
        assert_eq!(p.view().divisor,4); assert_eq!(p.view().dirty,[1,1,0]);
        let mut r=fixture(); r[0x70]=0; r[0x71]=7;
        let mut p=Primary::decode(r).unwrap(); p.common().unwrap();
        assert_eq!(p.view().current[0][0],8.); assert_eq!(p.view().current[1][0],4.);
        assert_eq!(p.sample(0.5).unwrap()[0][0],8.);
    }
    #[test]
    fn strict_threshold_copy_and_n_one_sampling() {
        for (threshold,expected) in [(8.,8.),(9.,2.),(-1.,8.),(0.,2.)] {
            let mut r=fixture(); put(&mut r,0x78,threshold);
            let mut p=Primary::decode(r).unwrap(); p.common().unwrap();
            assert_eq!(p.view().current[0][0],expected);
        }
        let mut r=fixture(); r[0x74]=1;
        let mut p=Primary::decode(r).unwrap();
        assert_eq!(p.sample(0.5).unwrap()[0][0],4.); // no n>1 guard in sampler
        p.common().unwrap(); assert_eq!(p.view().current[0][0],8.);
    }
    #[test]
    fn parameter_two_precedes_angle_and_preserves_up_opaque() {
        let mut r=fixture(); r[0xa8]=2; r[0x72]=1;
        put(&mut r,0x40,-0.); put(&mut r,0x44,2.); put(&mut r,0x48,1.);
        put(&mut r,0x64,0.25); put(&mut r,0x6c,-0.5);
        for at in [0x1c,0x30,0x73,0x80,0x84,0x8c,0x94,0x9c,0xac,0xc0,0xc3] {r[at]=0xa5;}
        let mut p=Primary::decode(r).unwrap(); p.common().unwrap(); p.sample(0.75).unwrap();
        assert_eq!(p.f(0x34).to_bits(),(-0.0f32).to_bits());
        assert_eq!(p.f(0x38),152.); assert_eq!(p.f(0x3c),0.9f32);
        assert_eq!(&p.raw()[0x64..0x70],&r[0x64..0x70]);
        for at in [0x1c,0x30,0x73,0x80,0x84,0x8c,0x94,0x9c,0xac,0xc0,0xc3] {
            assert_eq!(p.raw()[at],r[at]);
        }
        let mut high=r;put(&mut high,0x38,200.);
        let mut p=Primary::decode(high).unwrap();p.common().unwrap();assert_eq!(p.f(0x38),200.);
    }
    #[test]
    fn angular_retained_argument_store_order_and_atomic_missing_provider() {
        let mut r=fixture();r[0x72]=1;r[0x74]=3;
        put(&mut r,0x98,1.);put(&mut r,0xb0,2.);put(&mut r,0xb4,10.);put(&mut r,0xbc,20.);
        let mut p=Primary::decode(r).unwrap();assert!(p.common().is_err());assert_eq!(p.raw(),&r);
        // Scripted geometry provider tests the real angular arithmetic seam,
        // not native FSIN accuracy; no approximation is advertised as exact.
        struct Witness { calls: usize }
        impl AngularTrig for Witness {
            fn sine(&mut self, value: Sum)->Result<Sum> {
                self.calls+=1;
                let mut expected=Sum::from_f32(1.)?;expected.div(Sum::from_i32(3))?;
                assert_eq!(value.compare(expected),Ordering::Equal);
                assert_ne!(value.compare(Sum::from_f32(value.to_f32())?),Ordering::Equal);
                Sum::from_f32(0.5)
            }
            fn cosine(&mut self, value: Sum)->Result<Sum> {
                self.calls+=1;
                assert_eq!(value.compare(Sum::from_f32((1.0f64/3.) as f32)?),Ordering::Equal);
                Sum::from_f32(0.25)
            }
        }
        let mut w=Witness{calls:0};p.common_with_trig(&mut w).unwrap();
        assert_eq!(w.calls,2);assert_eq!(p.view().desired[0],[10.5,0.,21.]);
        assert_eq!(p.view().dirty,[1,1,1]); assert_eq!(p.view().divisor,3);
    }
    #[test]
    fn finite_validation_and_atomic_store_overflow() {
        for (at,bits) in [(0,13u32),(0x74,0),(0x74,u32::MAX),(0x34,f32::NAN.to_bits())] {
            let mut r=fixture();r[at..at+4].copy_from_slice(&bits.to_le_bytes());
            assert!(Primary::decode(r).is_err());
        }
        let mut r=fixture();r[0xa8]=2;put(&mut r,0x44,f32::MAX);
        // Adding150 to MAX cannot overflow at PC64, so force angular R*cos overflow instead.
        r[0xa8]=0;r[0x72]=1;put(&mut r,0xb0,f32::MAX);
        struct Overflow;
        impl AngularTrig for Overflow {
            fn sine(&mut self,_:Sum)->Result<Sum>{Sum::from_f32(2.)}
            fn cosine(&mut self,_:Sum)->Result<Sum>{Sum::from_f32(2.)}
        }
        let mut p=Primary::decode(r).unwrap();assert!(p.common_with_trig(&mut Overflow).is_err());
        assert_eq!(p.raw(),&r);
        for amount in [f32::NAN,-0.1,1.1] {assert!(p.sample(amount).is_err());assert_eq!(p.raw(),&r);}
    }
    #[test]
    fn binary32_nearest_even_residual_is_not_a_freeze_policy() {
        let mut r=fixture();r[0x74]=20;
        put(&mut r,4,f32::from_bits(0x42a3fff6)); put(&mut r,0x34,82.);
        let mut p=Primary::decode(r).unwrap();p.common().unwrap();
        assert_eq!(p.f(4).to_bits(),0x42a3fff6);
        assert_ne!(p.f(4),p.f(0x34));assert_eq!(p.view().dirty[0],1);
    }
    #[test]
    #[ignore="reads both SHA-pinned supplied saves as data; needs OPENMT2001_SAVE_ONE/TWO and sha256sum"]
    fn supplied_primary_state_common_and_fractional_regression() {
        use std::{io::Write,process::{Command,Stdio}};
        for (env,hash,payload_hash,mode,n) in [
            ("OPENMT2001_SAVE_ONE","2c3bfdbbdaeb2e12e08752e9f3ed05d6e5c4dc60aa48208113f46a84b329dded","6db51fca309761af0e5e9c43dfa40ed55c8c72a05fe15f10810629bbf51c918a",12,20),
            ("OPENMT2001_SAVE_TWO","4cf97da570b3dc1ab19508fa983b2ab12d418b0e08ea750f38640fcea7121931","820260e951da3f82ef09b9a8a020855624b21ba80f65f33f8995d272bf4691b7",6,10),
        ] {
            fn guard(bytes:&[u8],expected:&str) {
                let mut child=Command::new("sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
                child.stdin.take().unwrap().write_all(bytes).unwrap();
                let out=child.wait_with_output().unwrap();assert!(out.status.success());
                assert_eq!(std::str::from_utf8(&out.stdout).unwrap().split_whitespace().next().unwrap(),expected);
            }
            let bytes=std::fs::read(std::env::var(env).unwrap()).unwrap();guard(&bytes,hash);
            let save=crate::save::SaveFile::parse(&bytes).unwrap();let raw=*save.cameras().unwrap()[0];
            guard(&raw,payload_hash);let mut p=Primary::decode(raw).unwrap();let before=p.view();
            assert_eq!(before.mode,mode);assert_eq!(before.divisor,n);
            assert_eq!(before.dirty,[1,1,0]);assert_eq!(before.threshold,0.);
            assert_ne!(before.current,before.desired);assert_eq!(before.current,before.sampled);
            // These exact records are finite-store fixed points, not dispatch/input witnesses.
            p.common().unwrap();assert_eq!(p.view().current,before.current);
            for amount in [0.,0.25,0.5,1.] {assert_eq!(p.sample(amount).unwrap(),before.sampled);}
            assert_eq!(p.raw(),&raw,"all bytes retained at these numeric fixed points");
        }
    }
}

#[cfg(test)]
mod mode12_tests {
    use super::*;
    fn put(r:&mut [u8;196],at:usize,v:f32){r[at..at+4].copy_from_slice(&v.to_le_bytes());}
    fn state()->Primary {
        let mut r=[0;196];r[0]=12;r[0x70]=1;r[0x71]=1;r[0x74]=20;
        for (at,v) in [(0x38,120.),(0x68,1.),(0xb0,2.),(0xb4,10.),(0xb8,0.25),(0xbc,20.)] {put(&mut r,at,v);}
        Primary::decode(r).unwrap()
    }
    fn input()->Mode12Input {
        Mode12Input {build_visible:false,enable_translation:true,enable_keyboard:true,
            enable_rotation:true,enable_zoom:true,buttons:[false;2],delta:[0;2],keys:[false;4],
            sensitivity:50,legacy_yaw:false,invert_vertical:false,low_height:false}
    }
    // Scripted returns certify ONLY caller ordering/operands, not native trig.
    #[derive(Default)]
    struct Trig { calls:Vec<(char,u32)>, fail:Option<usize> }
    impl AngularTrig for Trig {
        fn sine(&mut self,a:Sum)->Result<Sum>{self.record('s',a)?;Sum::from_f32(-0.5)}
        fn cosine(&mut self,a:Sum)->Result<Sum>{self.record('c',a)?;Sum::from_f32(0.25)}
    }
    impl Trig {
        fn record(&mut self,c:char,a:Sum)->Result<()> {
            self.calls.push((c,a.to_f32().to_bits()));
            assert_eq!(a.compare(Sum::from_f32(a.to_f32())?),Ordering::Equal,
                "all callback trig operands reload rounded raw yaw");
            if self.fail==Some(self.calls.len()){return Err("scripted trig failure");} Ok(())
        }
    }
    #[test]
    fn disabled_still_emits_pose_from_old_cache_and_preserves_other_bytes() {
        let mut p=state();put(&mut p.raw,0x88,1.);put(&mut p.raw,0x90,0.5);
        for at in [0x1c,0x30,0x73,0x80,0x84,0x8c,0x94,0x9c,0xac,0xc0,0xc3] {p.raw[at]=0xa5;}
        let before=p.raw;
        let mut i=input();i.enable_translation=false;i.enable_keyboard=false;
        i.enable_rotation=false;i.enable_zoom=false;i.keys=[true;4];i.delta=[9,9];
        let mut t=Trig::default();assert!(p.mode12(i,&mut t).unwrap().is_empty());
        assert_eq!(t.calls,vec![('c',0),('s',0)]);
        assert_eq!(p.view().desired,[[35.5,120.,6.5],[35.,0.25,7.5]]);
        for at in 0..196 {
            if !(0x34..0x38).contains(&at) && !(0x3c..0x4c).contains(&at) {
                assert_eq!(p.raw[at],before[at],"unexpected write at {at:x}");
            }
        }
    }
    #[test]
    fn build_suppresses_translation_only_and_keeps_right_rotation_zoom() {
        let mut p=state();put(&mut p.raw,0xb4,600.);
        let mut i=input();i.build_visible=true;i.buttons=[true,true];i.delta=[4,5];i.keys=[true;4];
        let mut t=Trig::default();assert!(p.mode12(i,&mut t).unwrap().is_empty());
        assert_eq!(p.view().center,[600.,0.25,20.]);
        i.buttons=[false,true];i.delta=[14,2];
        let old_pose_x=p.f(0x34);let mut t=Trig::default();
        assert_eq!(p.mode12(i,&mut t).unwrap(),vec![CursorRequest{state:2,priority:1};2]);
        assert_eq!(t.calls.iter().map(|c|c.0).collect::<Vec<_>>(),vec!['c','s','s','c']);
        assert_eq!(t.calls[2].1,p.f(0x7c).to_bits());assert_eq!(t.calls[3].1,p.f(0x7c).to_bits());
        assert_eq!(p.f(0x98).to_bits(),p.f(0x7c).to_bits());
        assert_ne!(p.f(0x7c),0.);assert_eq!(p.f(0x38),118.);assert_eq!(p.f(0x44),-0.75);
        assert_eq!(p.f(0x34),old_pose_x,"no second pose after yaw/radius changes");
        assert_eq!(p.f(0xb4),600.);assert_eq!(p.view().dirty,[1,1,0]);
    }
    #[test]
    fn pan_signed_axes_and_keyboard_rounding_order_then_clamp() {
        for (delta,expected) in [([2,0],[6.,22.]),([-2,0],[14.,18.]),
            ([0,-2],[8.,16.]),([0,2],[12.,24.])] {
            let mut p=state();put(&mut p.raw,0x88,1.);put(&mut p.raw,0x90,0.5);
            let mut i=input();i.buttons=[true,true];i.delta=delta;
            let e=p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(e,vec![CursorRequest{state:3,priority:1}]);
            assert_eq!([p.f(0xb4),p.f(0xbc)],expected);
            assert_eq!(p.f(0x40),35.,"pose used pre-pan center");
        }
        for (index,expected) in [[8.,21.],[12.,19.],[11.,22.],[9.,18.]].into_iter().enumerate() {
            let mut p=state();put(&mut p.raw,0x88,1.);put(&mut p.raw,0x90,0.5);
            let mut i=input();i.keys[index]=true;p.mode12(i,&mut Trig::default()).unwrap();
            assert_eq!([p.f(0xb4),p.f(0xbc)],expected);
        }
        let mut p=state();put(&mut p.raw,0xb4,256.);put(&mut p.raw,0x88,-f32::from_bits(0x37000000));
        // At256 the upward ULP is twice the downward one. Sequential
        // Right/Left rounded stores differ from pre-canceling opposing keys.
        let mut i=input();i.keys=[true,true,false,false];
        p.mode12(i,&mut Trig::default()).unwrap();assert!(p.f(0xb4)<256.);
        let mut p=state();put(&mut p.raw,0xb4,501.);put(&mut p.raw,0xbc,-501.);
        p.mode12(input(),&mut Trig::default()).unwrap();assert_eq!([p.f(0xb4),p.f(0xbc)],[500.,-500.]);
        let mut i=input();i.enable_keyboard=false;i.enable_rotation=true;i.buttons=[false,true];i.delta=[14,0];
        let mut t=Trig::default();p.mode12(i,&mut t).unwrap();
        assert_eq!(t.calls.len(),2,"disabled keyboard re-read jumps directly to zoom, not rotation");
    }
    #[test]
    fn integer_yaw_branches_and_atomic_domain_or_provider_errors() {
        let mut i=input();i.buttons=[false,true];i.delta=[99,0];i.legacy_yaw=true;
        let mut p=state();p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x7c),0.,"28/100 truncates to0");
        i.sensitivity=-49;let mut p=state();p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x7c),28.);
        i.legacy_yaw=false;i.sensitivity=198;let mut p=state();p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x7c),-14.);
        for (legacy,speed) in [(true,-50),(false,200)] {
            let mut p=state();let before=p.clone();i.legacy_yaw=legacy;i.sensitivity=speed;
            assert!(p.mode12(i,&mut Trig::default()).is_err());assert_eq!(p,before);
        }
        for fail in [1,2,3,4] {
            let mut p=state();let before=p.clone();let mut t=Trig{fail:Some(fail),..Default::default()};
            i=input();i.buttons=[false,true];i.delta=[1,0];
            assert!(p.mode12(i,&mut t).is_err());assert_eq!(p,before);
        }
        for (buttons,delta,invert) in [([true,true],[i32::MIN,0],false),
            ([false,true],[0,i32::MIN],false),([false,true],[0,i32::MIN],true)] {
            let mut p=state();let before=p.clone();let mut i=input();i.buttons=buttons;i.delta=delta;i.invert_vertical=invert;
            assert!(p.mode12(i,&mut Trig::default()).is_err());assert_eq!(p,before);
        }
        let mut p=state();p.raw[0]=6;let before=p.clone();let mut t=Trig::default();
        assert!(p.mode12(input(),&mut t).is_err());assert!(t.calls.is_empty());assert_eq!(p,before);
    }
    #[test]
    fn zoom_equality_low_height_invert_and_parameter_two() {
        for (dy,height) in [(2,27.),(-2,198.)] {
            let mut p=state();put(&mut p.raw,0x38,height);let radius=p.f(0xb0);
            let mut i=input();i.buttons=[false,true];i.delta=[0,dy];
            p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x38),height);
            assert_eq!(p.f(0xb0),radius,"strict equality rejects entire zoom");
        }
        let mut p=state();put(&mut p.raw,0x38,20.);let mut i=input();
        i.buttons=[false,true];i.delta=[0,2];i.low_height=true;
        p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x38),18.);assert_eq!(p.f(0xb0),2.2);
        let mut p=state();i.low_height=false;i.invert_vertical=true;
        p.mode12(i,&mut Trig::default()).unwrap();assert_eq!(p.f(0x38),122.);assert_eq!(p.f(0xb0),1.8);
        let mut p=state();p.raw[0xa8]=2;put(&mut p.raw,0x38,1.);
        i.delta=[14,10];i.sensitivity=200;i.invert_vertical=true;
        let mut t=Trig::default();let effects=p.mode12(i,&mut t).unwrap();
        assert_eq!(p.f(0x38),-9.,"special zoom ignores invert, speed and normal height gates");
        assert_eq!(p.f(0x44),0.25);assert_eq!(p.f(0xb0),2.);assert_eq!(t.calls.len(),2);
        assert_eq!(effects,vec![CursorRequest{state:2,priority:1}]);
    }
}
