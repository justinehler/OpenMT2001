//! Original479040..479bc0 scoring stages over retained world data.
use super::{Call, Reply, base, limits};
use crate::{
    ai_profiles as profile, ai_strategy_queries as q, ai_valuation_queries as v,
    economy::ResidentAccounts,
    pe::PeImage,
    player_manager::{Player, word},
    player_messages::Players,
    x87::Sum,
};
pub struct World<'a> {
    pub image: &'a PeImage<'a>,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub native_utilities: &'a [f32; 4],
    pub loan_status: &'a [u8; 6],
    pub hour: i32,
    pub residents: &'a ResidentAccounts,
    pub commodity_active: &'a [bool; 32],
    pub before: &'a [Player],
    pub after: &'a [Player],
}
struct Views<'a> {
    before: &'a [Player],
    current: &'a mut Player,
    after: &'a [Player],
}
impl Players for Views<'_> {
    fn get(&self, slot: usize) -> Option<&Player> {
        if slot < self.before.len() {
            self.before.get(slot)
        } else if slot == self.before.len() {
            Some(self.current)
        } else {
            self.after.get(slot - self.before.len() - 1)
        }
    }
    fn get_mut(&mut self, slot: usize) -> Option<&mut Player> {
        if slot == self.before.len() {
            Some(self.current)
        } else {
            None
        }
    }
}
impl World<'_> {
    fn player<'b>(&'b self, current: &'b Player, slot: usize) -> Option<&'b Player> {
        if slot < self.before.len() {
            self.before.get(slot)
        } else if slot == self.before.len() {
            Some(current)
        } else {
            self.after.get(slot - self.before.len() - 1)
        }
    }
    fn row(&self, category: u32) -> Result<&[u8; 380], String> {
        self.categories
            .get(category as usize)
            .ok_or_else(|| "valuation category missing".into())
    }
    fn value(&self, category: u32) -> Result<Sum, String> {
        crate::ai_messages::offers::execution::category_value(self.row(category)?, self.image)
    }
    fn investment(&self, p: &Player, category: u32) -> Result<i32, String> {
        if q::utility_or_transport(self.image, category)? {
            v::utility_investment(
                p,
                category,
                self.categories,
                self.buildings,
                self.businesses,
                self.catalog,
                self.native_utilities,
                self.image,
            )
        } else {
            v::business_sum(self.row(category)?, self.buildings, self.businesses)
        }
    }
    fn loan(&self, p: &Player) -> Result<bool, String> {
        Ok(*self
            .loan_status
            .get(p.bytes[0x1bc] as usize)
            .ok_or("valuation loan slot missing")?
            != 0)
    }
    fn remaining(&self, p: &Player) -> Result<i32, String> {
        Ok(if self.loan(p)? {
            let n = 24i32.wrapping_sub(self.hour);
            if n == 0 { 1 } else { n }
        } else {
            0
        })
    }
    fn character(&self, p: &Player) -> u32 {
        word(&p.bytes, 0x150)
    }
    fn cash(&self, p: &Player) -> Result<i32, String> {
        Ok(Sum::from_f32(f32::from_bits(word(&p.bytes, 4)))?.ftol_low_word())
    }
    fn factor(&self, p: &Player, address: u32) -> Result<Sum, String> {
        Sum::from_f32(profile::factor(self.image, address, self.character(p))?).map_err(Into::into)
    }
    fn loan_penalty(&self, p: &Player) -> Result<i32, String> {
        let n = self.remaining(p)?;
        Ok(if n > 0 {
            profile::loan_multiplier(self.character(p)).wrapping_mul(50i32.wrapping_sub(n))
        } else {
            0
        })
    }
    fn cash_rank(&self, p: &Player, slot: u32) -> Result<i32, String> {
        let adjusted = |p: &Player| -> Result<i32, String> {
            Ok(self.cash(p)?.wrapping_add(if self.loan(p)? {
                0
            } else {
                v::income_margin(p)?
            }))
        };
        let own = adjusted(p)?;
        let mut rank = 1;
        for i in 0..6 {
            if let Some(other) = self.player(p, i)
                && other.bytes[0x10] != 0
                && u32::from(other.bytes[0x1bc]) != slot
                && adjusted(other)? > own
            {
                rank += 1;
            }
        }
        Ok(rank)
    }
    fn strongest_business_cash(
        &self,
        p: &Player,
        slot: u32,
        category: u32,
    ) -> Result<bool, String> {
        let row = self.row(category)?;
        let own = self.cash(p)?.wrapping_add(
            v::owned_businesses(slot, row, self.buildings, self.businesses)?.wrapping_mul(100),
        );
        for i in 0..6 {
            let Some(other) = self.player(p, i) else {
                continue;
            };
            if other.bytes[0x10] == 0 || u32::from(other.bytes[0x1bc]) == slot {
                continue;
            }
            let count = v::owned_businesses(i as u32, row, self.buildings, self.businesses)?;
            if count > 0 && self.cash(other)?.wrapping_add(count.wrapping_mul(100)) > own {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn resource(&self, p: &Player, category: u32) -> Result<i32, String> {
        let invested = self.investment(p, category)?;
        let adjustment = profile::adjustment(self.character(p));
        let mut value = self.value(category)?;
        value.add(Sum::from_i32(adjustment));
        value.add(Sum::from_i32(invested).negated());
        Ok(value.ftol_low_word())
    }
    fn investment_score(&self, p: &Player, category: u32, input: f32) -> Result<Sum, String> {
        let size = v::group_size(self.image, category)?;
        let small = v::owned_area(p, self.row(category)?, self.buildings, self.businesses)? < 24;
        let any = v::any_development(self.image, self.categories, self.buildings, self.businesses)?;
        let mut amount = self.investment(p, category)?;
        let mut stored = 0.0;
        if (2..=4).contains(&size) {
            if !small && !any {
                let mut minimum = self.factor(p, 0x47cf00)?;
                minimum.mul_f32(self.image.f32_at(match size {
                    2 => 0x525ca8,
                    3 => 0x525178,
                    _ => 0x525cc8,
                })?)?;
                amount = amount.max(minimum.ftol_low_word());
            }
            let mut value = Sum::from_i32(amount);
            for at in [0x525058, 0x525cc4, 0x525078, 0x5250e0] {
                value.mul_f32(self.image.f32_at(at)?)?;
            }
            stored = value.to_f32();
        }
        let mut value = self.factor(p, 0x47cf00)?;
        value.mul_f32(stored)?;
        value.add_f32(input)?;
        Ok(value)
    }
    fn history_score(&self, p: &Player, category: u32, input: f32) -> Result<Sum, String> {
        let mut amount = Sum::from_i32(v::history_sum(
            self.row(category)?,
            self.buildings,
            self.businesses,
            self.image,
        )?)
        .to_f32();
        let value = self.value(category)?.to_f32();
        if amount < self.image.f32_at(0x525040)? {
            amount = 0.0;
        }
        if amount > value {
            amount = value;
        }
        let mut ratio = Sum::from_f32(amount)?;
        ratio.mul_f32(self.image.f32_at(0x525058)?)?;
        ratio.div_f32(value).map_err(|e| {
            format!("auction valuation category{category} amount{amount} value{value}: {e}")
        })?;
        ratio.mul_f32(self.image.f32_at(0x52524c)?)?;
        ratio.mul(self.factor(p, 0x47cf90)?);
        ratio.add_f32(input)?;
        Ok(ratio)
    }
    fn ownership_score(
        &self,
        p: &mut Player,
        auction: bool,
        slot: u32,
        category: u32,
        input: f32,
    ) -> Result<Sum, String> {
        let size = v::group_size(self.image, category)?;
        let area = q::occupied_area(self.row(category)?, self.buildings)?;
        let mut area_score = Sum::from_i32(area);
        let stored = if matches!(size, 2 | 3) {
            for at in [0x525058, 0x525cd0, 0x525094, 0x5250e0] {
                area_score.mul_f32(self.image.f32_at(at)?)?;
            }
            area_score.to_f32()
        } else {
            0.0
        };
        let mut modifier = self.factor(p, 0x47d010)?;
        modifier.mul_f32(stored)?;
        let modifier = modifier.to_f32();
        let mut owned =
            v::owned_businesses(slot, self.row(category)?, self.buildings, self.businesses)?;
        let views = Views {
            before: self.before,
            current: p,
            after: self.after,
        };
        let (_, richest) = v::richest_other(slot, &views);
        let cash = self.cash(p)?;
        let remaining = self.remaining(p)?;
        let rank = self.cash_rank(p, slot)?;
        let reserve = profile::reserve(self.character(p));
        let reduction = if remaining > 0 {
            reserve.wrapping_add(28i32.wrapping_sub(remaining).wrapping_mul(100))
        } else {
            0
        };
        let score = if owned > 0 {
            let mut ratio = Sum::from_i32(owned);
            let mut denominator = Sum::from_i32(rank);
            denominator.add_f32(self.image.f32_at(0x525034)?)?;
            ratio.div(denominator)?;
            ratio.add(Sum::from_f64(f64::from_le_bytes(
                self.image.at(0x5251b8, 8)?.try_into().unwrap(),
            ))?);
            let initial = ratio.ftol_low_word();
            if !auction && reserve.wrapping_sub(reduction).wrapping_add(cash) < richest {
                let mut cost = Sum::from_i32(reduction.wrapping_sub(cash).wrapping_add(richest));
                cost.mul_f32(self.image.f32_at(0x525ccc)?)?;
                cost.add_f32(self.image.f32_at(0x525034)?)?;
                owned = owned.wrapping_sub(cost.ftol_low_word());
                if owned <= 0 {
                    owned.wrapping_mul(10).max(-60)
                } else {
                    0
                }
            } else {
                initial
            }
        } else if !auction && !self.strongest_business_cash(p, slot, category)? {
            -40
        } else {
            0
        };
        let mut result = Sum::from_i32(score);
        result.add_f32(modifier)?;
        result.add_f32(input)?;
        Ok(result)
    }
    fn group_budget(&self, p: &Player, category: u32) -> Result<i32, String> {
        let penalty = self.loan_penalty(p)?;
        let cash = f32::from_bits(word(&p.bytes, 4));
        let invested = Sum::from_i32(v::business_sum(
            self.row(category)?,
            self.buildings,
            self.businesses,
        )?)
        .to_f32();
        let mut value = Sum::from_i32(v::income_margin(p)?);
        value.add_f32(invested)?;
        value.add_f32(cash)?;
        let value = value.ftol_low_word();
        Ok(if penalty > 0 {
            value.wrapping_sub(penalty)
        } else {
            value.wrapping_add(profile::reserve(self.character(p)))
        })
    }
    fn desired_budget(&self, p: &Player, category: u32) -> Result<Sum, String> {
        let half = self.value(category)?.ftol_low_word() / 2;
        let amount = self.investment(p, category)?;
        let mut extra = if amount.wrapping_mul(2) >= half {
            half.wrapping_mul(4)
        } else {
            amount.wrapping_mul(8)
        };
        extra = extra.max(profile::reserve(self.character(p)) / 2);
        let mut value = self.value(category)?;
        value.add(Sum::from_i32(extra));
        Ok(value)
    }
    fn prestige_score(&self, p: &Player, category: u32, input: f32) -> Result<Sum, String> {
        let desired = profile::preferred_tier(self.character(p));
        let actual = Sum::from_f32(f32::from_bits(word(self.row(category)?, 0x84)))?
            .ftol_low_word()
            .min(5);
        let difference = desired.wrapping_sub(actual).wrapping_abs();
        let mut value = Sum::from_i32(2i32.wrapping_sub(difference));
        value.add_f32(input)?;
        Ok(Sum::from_i32(value.ftol_low_word()))
    }
    fn liquidity_score(
        &self,
        p: &Player,
        category: u32,
        input: f32,
        base: f32,
    ) -> Result<Sum, String> {
        let cash = self.cash(p)?.wrapping_sub(self.loan_penalty(p)?);
        let required = self.resource(p, category)?;
        let result = if cash > required {
            let mut amount = Sum::from_i32(cash.wrapping_sub(required));
            let cap = Sum::from_f32(self.image.f32_at(0x525cb4)?)?;
            if amount.compare(cap).is_gt() {
                amount = cap;
            }
            amount.mul_f32(self.image.f32_at(0x525058)?)?;
            amount.mul_f32(self.image.f32_at(0x525cd4)?)?;
            let mut gain = Sum::from_i32(amount.ftol_low_word());
            gain.mul_f32(input)?;
            gain.mul_f32(self.image.f32_at(0x5250e0)?)?;
            let mut result = Sum::from_i32(gain.ftol_low_word());
            result.add_f32(base)?;
            result.ftol_low_word()
        } else {
            let ratio = required.wrapping_sub(cash).min(4000).wrapping_mul(100) / 6000;
            let mut reduction = Sum::from_i32(ratio);
            reduction.mul_f32(input)?;
            reduction.mul_f32(self.image.f32_at(0x5250e0)?)?;
            let mut result = Sum::from_f32(base)?;
            result.add(Sum::from_i32(reduction.ftol_low_word()).negated());
            result.ftol_low_word()
        };
        Ok(Sum::from_i32(result))
    }
    fn lease_score(
        &self,
        p: &Player,
        category: u32,
        input: f32,
        resource: i32,
    ) -> Result<Sum, String> {
        let ids = v::category_group(self.image, category)?;
        let mut shortest = 120i32;
        for id in ids {
            if id >= 0 {
                let row = self.row(id as u32)?;
                if row[5] != 0 {
                    shortest = shortest.min(word(row, 0x7c) as i32);
                }
            }
        }
        let size = v::group_size(self.image, category)?;
        let owner = self.row(category)?[5] as i8;
        let cash = if owner > 0 {
            self.cash(
                self.player(p, owner as usize - 1)
                    .ok_or("valuation owner missing")?,
            )?
        } else {
            0
        };
        let state = word(&p.bytes, 0x1e30);
        let mut deduction = 0;
        if size <= 3 && shortest < 5 && matches!(state, 0 | 2) {
            let at = match shortest {
                1 => Some(0x52506c),
                2 => Some(0x525078),
                3 if cash < resource => Some(0x525064),
                4 if cash < resource => Some(0x5250a0),
                _ => None,
            };
            if let Some(at) = at {
                let mut amount = Sum::from_f32(input)?;
                amount.mul_f32(self.image.f32_at(at)?)?;
                amount.mul_f32(self.image.f32_at(0x5250e0)?)?;
                deduction = amount.ftol_low_word();
            }
        }
        let mut result = Sum::from_f32(input)?;
        result.add(Sum::from_i32(deduction).negated());
        let result = result.ftol_low_word();
        Ok(Sum::from_i32(if result > 0 {
            result
        } else {
            Sum::from_f32(input)?.ftol_low_word()
        }))
    }
    fn poll_score(&self, p: &mut Player, category: u32, input: f32) -> Result<Sum, String> {
        crate::consumer_poll::start(
            p,
            category as i32,
            self.categories,
            self.buildings,
            self.businesses,
            self.residents,
            self.commodity_active,
        )?;
        crate::consumer_poll::stop(p);
        let count = u16::from_le_bytes(p.bytes[0x20a..0x20c].try_into().unwrap());
        crate::consumer_poll::clear(p);
        let mut scaled = Sum::from_i32(i32::from(count));
        scaled.mul_f32(self.image.f32_at(0x525b3c)?)?;
        let mut value = self.factor(p, 0x47d1d0)?;
        value.mul(Sum::from_i32(scaled.ftol_low_word()));
        let mut value = Sum::from_i32(value.ftol_low_word());
        value.add_f32(input)?;
        Ok(Sum::from_i32(value.ftol_low_word()))
    }
    pub fn dispatch(&self, call: Call, p: &mut Player) -> Result<Reply, String> {
        let a = call.args;
        let (slot, category) = match call.address {
            0x4477e0 => (self.before.len() as u32, a[0]),
            0x4790c0 => (a[0], a[2]),
            0x479150 | 0x479c60 => (a[0], 0),
            0x4793c0 => (a[0], a[1]),
            0x479460 => (a[1], a[2]),
            _ => (a[1], a[0]),
        };
        if slot as usize != self.before.len() {
            return Err("valuation scoring caller slot differs from current player".into());
        }
        let mut reply = Reply::default();
        reply.extended = match call.address {
            0x4477e0 => self.value(category)?,
            0x4790c0 => {
                let mut views = Views {
                    before: self.before,
                    current: p,
                    after: self.after,
                };
                base::evaluate(
                    &mut base::World {
                        image: self.image,
                        categories: self.categories,
                        buildings: self.buildings,
                        players: &mut views,
                        loan_status: self.loan_status,
                    },
                    slot,
                    a[1] != 0,
                    category,
                )?
            }
            0x479040 => {
                reply.integer = self.resource(p, category)? as u32;
                Sum::default()
            }
            0x479150 => limits::affordability(p, f32::from_bits(a[1]), a[2] as i32, self.image)?,
            0x478ff0 => {
                reply.integer = if !q::utility_or_transport(self.image, category)?
                    && *p
                        .bytes
                        .get(0x178 + category as usize)
                        .ok_or("valuation category flag missing")?
                        != 0
                    && q::occupied_area(self.row(category)?, self.buildings)? == 0
                {
                    52
                } else {
                    0
                };
                Sum::default()
            }
            0x479280 => self.investment_score(p, category, f32::from_bits(a[2]))?,
            0x4793c0 => self.history_score(p, category, f32::from_bits(a[2]))?,
            0x479460 => self.ownership_score(p, a[0] != 0, slot, category, f32::from_bits(a[3]))?,
            0x479640 => {
                reply.integer = self.group_budget(p, category)? as u32;
                Sum::default()
            }
            0x4796f0 => self.desired_budget(p, category)?,
            0x4797a0 => self.prestige_score(p, category, f32::from_bits(a[2]))?,
            0x479810 => {
                self.liquidity_score(p, category, f32::from_bits(a[2]), f32::from_bits(a[3]))?
            }
            0x4799c0 => self.lease_score(p, category, f32::from_bits(a[2]), a[3] as i32)?,
            0x479bc0 => self.poll_score(p, category, f32::from_bits(a[2]))?,
            0x479c60 => {
                reply.integer =
                    limits::final_budget(p, a[1] as i32, a[2] as i32, self.loan(p)?, self.image)?
                        as u32;
                Sum::default()
            }
            _ => {
                return Err(format!(
                    "valuation scoring helper {:08x} unbound",
                    call.address
                ));
            }
        };
        Ok(reply)
    }
}
#[test]
#[ignore = "requires original complete46d7a0 valuation capture and executable"]
fn matches_native_complete_valuation() {
    fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (a, b) = data.split_at(n);
        *data = b;
        a
    }
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_VALUATION_COMPLETE").unwrap())
            .unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut data = &bytes[..];
    assert_eq!(take(&mut data, 8), b"MTAVALL1");
    assert_eq!(word(take(&mut data, 4), 0), 0x37f);
    let count = word(take(&mut data, 4), 0);
    for case in 0..count {
        let category = word(take(&mut data, 4), 0);
        let slot = word(take(&mut data, 4), 0) as usize;
        let mode = word(take(&mut data, 4), 0);
        let hour = word(take(&mut data, 4), 0) as i32;
        let status: &[u8; 6] = take(&mut data, 6).try_into().unwrap();
        let sentinel = take(&mut data, 1)[0];
        let categories: Box<[[u8; 380]; 52]> = take(&mut data, 52 * 380)
            .chunks_exact(380)
            .map(|r| r.try_into().unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap();
        let mut buildings = vec![[0u8; 916]; 600];
        for (out, b) in buildings
            .iter_mut()
            .zip(take(&mut data, 16 * 916).chunks_exact(916))
        {
            out.copy_from_slice(b);
        }
        buildings[599][0x24d] = sentinel;
        let mut businesses = vec![[0u8; 544]; 600];
        for (out, b) in businesses
            .iter_mut()
            .zip(take(&mut data, 16 * 544).chunks_exact(544))
        {
            out.copy_from_slice(b);
        }
        let rows = take(&mut data, 45 * 116);
        let catalog = crate::business_pool::BusinessCatalog::from_rows(std::array::from_fn(|i| {
            rows[i * 116..(i + 1) * 116].try_into().unwrap()
        }));
        let utilities = std::array::from_fn(|_| f32::from_bits(word(take(&mut data, 4), 0)));
        let nodes = take(&mut data, 36 * 330)
            .chunks_exact(330)
            .map(|r| crate::resident::Resident::from_bytes(r.try_into().unwrap()))
            .collect();
        let accounts = take(&mut data, 36 * 144)
            .chunks_exact(144)
            .map(|r| crate::resident::SpendingAccount::from_bytes(r.try_into().unwrap()))
            .collect();
        let residents = ResidentAccounts {
            active_count: 36,
            nodes,
            accounts,
        };
        let raw = take(&mut data, 32);
        let active = std::array::from_fn(|i| raw[i] != 0);
        let mut players: Vec<Player> = (0..6)
            .map(|i| Player {
                reference: 0x10010000 + i * 0x2000,
                bytes: take(&mut data, 0x1e50).to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let after = take(&mut data, 6 * 0x1e50);
        let expected_integer = word(take(&mut data, 4), 0);
        let expected_float = word(take(&mut data, 4), 0);
        let length = word(take(&mut data, 4), 0);
        let traces: Vec<Vec<u32>> = (0..length)
            .map(|_| {
                let n = word(take(&mut data, 4), 0);
                (0..n).map(|_| word(take(&mut data, 4), 0)).collect()
            })
            .collect();
        let (before, current) = players.split_at_mut(slot);
        let (current, after_players) = current.split_first_mut().unwrap();
        let world = World {
            image: &image,
            categories: &categories,
            buildings: &buildings,
            businesses: &businesses,
            catalog: &catalog,
            native_utilities: &utilities,
            loan_status: status,
            hour,
            residents: &residents,
            commodity_active: &active,
            before,
            after: after_players,
        };
        let mut output = 0;
        let mut index = 0;
        let returned = super::evaluate(
            current,
            mode,
            Some(category),
            image.f32_at(0x5250e0).unwrap(),
            &mut output,
            |call, p| {
                let t = &traces[index];
                assert_eq!(call.address, t[0], "case {case} helper {index}");
                assert_eq!(call.count, t[1] as usize);
                assert_eq!(
                    &call.args[..call.count],
                    &t[2..],
                    "case {case} helper {index} {:08x}",
                    call.address
                );
                index += 1;
                world.dispatch(call, p)
            },
        )
        .unwrap_or_else(|err| panic!("case {case} helper {index}: {err}"));
        assert_eq!(index, traces.len());
        assert_eq!(
            output as u32, expected_integer,
            "case {case} integer result"
        );
        assert_eq!(
            returned.to_f32().to_bits(),
            expected_float,
            "case {case} f32 result"
        );
        for (i, p) in players.iter().enumerate() {
            assert_eq!(
                &p.bytes[..],
                &after[i * 0x1e50..(i + 1) * 0x1e50],
                "case {case} player {i}"
            );
        }
    }
    assert!(data.is_empty());
}
