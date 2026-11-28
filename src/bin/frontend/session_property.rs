//! Human property offer admission and synchronous retained message routing.
use super::{CommandAssets, Session};
use openmt2001::{live_commands, player_messages, property_offer};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inspection {
    pub category: u32,
    pub owned: bool,
    pub panel: property_offer::Panel,
    pub submitted: bool,
}
impl Session<'_> {
    pub fn respond_outcome(
        &mut self,
        context: u32,
        assets: &CommandAssets<'_, '_>,
        mut event: impl FnMut(openmt2001::outcome_notice::Response) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut responses = Vec::new();
        openmt2001::outcome_notice::respond(
            context,
            self.catalog
                .selected_level()
                .ok_or("outcome selected level identity unresolved")?
                .id(),
            &openmt2001::outcome_notice::World {
                image: assets.image,
                players: &self.roster.manager.players,
                current_reference: self.roster.status.current_player,
                objective: &self.world.city.objective,
            },
            |response| {
                responses.push(response);
                Ok(())
            },
        )?;
        let mut characters = None;
        for response in responses {
            match response {
                openmt2001::outcome_notice::Response::Characters { winner, loser } => {
                    characters = Some((winner, loser));
                }
                openmt2001::outcome_notice::Response::Sequence(sequence) => {
                    self.begin_result_sequence(
                        sequence,
                        characters.ok_or("Result sequence character assignment missing")?,
                        assets,
                    )?;
                }
                openmt2001::outcome_notice::Response::Unlock(id) => self.catalog.unlock(id),
                openmt2001::outcome_notice::Response::Music { .. } => (),
            }
            event(response)?;
        }
        Ok(())
    }
    pub fn inspect_property_rights(
        &self,
        category: u32,
        panel_mode: u8,
    ) -> Result<openmt2001::property_rights::Inspection, String> {
        let row = self
            .world
            .city
            .categories
            .get(category as usize)
            .ok_or("rights category outside pool")?;
        let owner = i32::from(row[5] as i8) - 1;
        let owner_reference = if row[5] == 0 {
            0
        } else {
            self.roster
                .manager
                .players
                .get(owner as usize)
                .ok_or("rights owner outside roster")?
                .reference
        };
        Ok(openmt2001::property_rights::inspect(
            Some(row),
            owner_reference,
            self.roster.status.current_player,
            panel_mode,
        ))
    }
    /// The caller supplies the original property panel+509 mode byte. Admission
    /// uses404eb0's visible button; the command consumer retains its25-tick delay.
    pub fn activate_property_rights(
        &mut self,
        category: u32,
        panel_mode: u8,
    ) -> Result<openmt2001::property_rights::Inspection, String> {
        if self.connected.is_some() {
            return Err(openmt2001::network::Error::Unsupported.to_string());
        }
        let inspection = self.inspect_property_rights(category, panel_mode)?;
        if !inspection.activate_visible {
            return Ok(inspection);
        }
        let mut selected = Some(category);
        openmt2001::property_rights::activate(
            &mut self.world.city.categories,
            &mut selected,
            [0; 24],
            |packet, _, _| {
                let mut descriptor = [0; 52];
                descriptor[..24].copy_from_slice(&packet);
                self.live_commands
                    .submit(
                        live_commands::Command::Auction { descriptor },
                        self.world.clock.timing[0],
                        true,
                        |_| Err("rights delay wrapped into immediate dispatch".into()),
                    )
                    .map(|_| ())
            },
        )?;
        self.inspect_property_rights(category, panel_mode)
    }
    pub fn inspect_property_offer(
        &mut self,
        category: u32,
        amount: Option<f32>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<Inspection, String> {
        let mut panel = property_offer::Panel::default();
        if let Some(amount) = amount {
            panel.amount = amount;
        }
        self.inspect_property_offer_panel(category, panel, assets)
    }
    /// The host owns original widget state; changing category preserves the
    /// amount/range unless404d90 itself updates them.
    pub fn inspect_property_offer_panel(
        &mut self,
        category: u32,
        mut panel: property_offer::Panel,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<Inspection, String> {
        self.roster.publish_accounts(&self.world.city)?;
        let current = self
            .roster
            .manager
            .players
            .iter_mut()
            .find(|p| p.reference == self.roster.status.current_player)
            .ok_or("property current player unresolved")?;
        let row = self
            .world
            .city
            .categories
            .get(category as usize)
            .ok_or("property category outside pool")?;
        let owned = i32::from(row[5] as i8) - 1 == i32::from(current.bytes[0x1bc]);
        let mut world = property_offer::World {
            categories: &mut self.world.city.categories,
            current,
            selected: Some(category),
            allowance: i32::from_le_bytes(assets.image.at(0x541c7c, 4)?.try_into().unwrap()),
            affordability_override: false,
        };
        property_offer::refresh(&mut panel, &mut world, assets.image, |_, _, _| Ok(()))?;
        Ok(Inspection {
            category,
            owned,
            panel,
            submitted: false,
        })
    }
    pub fn inspect_property_offer_display(
        &mut self,
        category: u32,
        mut panel: property_offer::Panel,
        initialize_amount: bool,
        auction_visible: bool,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<Inspection, String> {
        if initialize_amount {
            let row = self
                .world
                .city
                .categories
                .get(category as usize)
                .ok_or("property category outside pool")?;
            panel.amount = property_offer::selected_amount(row, assets.image)?;
        }
        let mut inspection = self.inspect_property_offer_panel(category, panel, assets)?;
        let current = self
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == self.roster.status.current_player)
            .ok_or("property current player unresolved")?;
        property_offer::display(
            &mut inspection.panel,
            &self.world.city.categories[category as usize],
            current.bytes[0x1bc],
            auction_visible,
            assets.image,
        )?;
        Ok(inspection)
    }
    pub fn submit_property_offer(
        &mut self,
        category: u32,
        amount: f32,
        assets: &CommandAssets<'_, '_>,
        event: impl FnMut(live_commands::Event) -> Result<(), String>,
    ) -> Result<Inspection, String> {
        let panel = property_offer::Panel {
            amount,
            ..Default::default()
        };
        self.submit_property_offer_panel(category, panel, assets, event)
    }
    pub fn submit_property_offer_panel(
        &mut self,
        category: u32,
        panel: property_offer::Panel,
        assets: &CommandAssets<'_, '_>,
        mut event: impl FnMut(live_commands::Event) -> Result<(), String>,
    ) -> Result<Inspection, String> {
        if self.connected.is_some() {
            return Err(openmt2001::network::Error::Unsupported.to_string());
        }
        let amount = panel.amount;
        let mut inspection = self.inspect_property_offer_panel(category, panel, assets)?;
        if inspection.panel.visible == 0 {
            return Ok(inspection);
        }
        // Worker inputs must be representable by the original amount widget.
        if !amount.is_finite()
            || !inspection.owned
                && (amount < inspection.panel.minimum || amount > inspection.panel.maximum)
        {
            return Err("property offer amount outside native control limits".into());
        }
        let slot = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.roster.status.current_player)
            .ok_or("property current player unresolved")?;
        let remaining = u32::from_le_bytes(self.roster.manager.bytes[12..16].try_into().unwrap());
        let (before, tail) = self.roster.manager.players.split_at_mut(slot);
        let (current, after) = tail.split_first_mut().unwrap();
        let mut world = property_offer::World {
            categories: &mut self.world.city.categories,
            current,
            selected: Some(category),
            allowance: i32::from_le_bytes(assets.image.at(0x541c7c, 4)?.try_into().unwrap()),
            affordability_override: false,
        };
        let mut messages = player_messages::Context {
            arena: player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        let result = property_offer::submit(
            property_offer::Action::Confirm,
            &mut inspection.panel,
            &mut world,
            assets.image,
            |call, _, world| {
                match call {
                    property_offer::Call::Producer { address, arguments } => {
                        let args = openmt2001::ai_messages::offers::producers::property(
                            address,
                            world.current.bytes[0x1bc],
                            arguments[0],
                            arguments[1],
                            arguments[2],
                            world.categories,
                        )?;
                        inspection.submitted = property_offer::route(
                            args,
                            remaining,
                            &mut player_messages::SplitPlayers {
                                before,
                                current: world.current,
                                after,
                            },
                            &mut messages,
                            |dispatch| {
                                match dispatch{
                        property_offer::Dispatch::Queue{descriptor,..}=>self.live_commands.submit_with_delay(live_commands::Command::Auction{descriptor},self.world.clock.timing[0],descriptor[16]!=0,0,
                            |_|Err("property offer delay wrapped into immediate dispatch".into())).map(|_|()),
                        property_offer::Dispatch::External{route,packet}=>event(live_commands::Event::PlayerMessageRoute{route,packet}),
                    }
                            },
                        )?;
                    }
                    other => event(live_commands::Event::PropertyOffer(other))?,
                }
                Ok(())
            },
        );
        self.allocation_next = messages.arena.next;
        result?;
        Ok(inspection)
    }
}
