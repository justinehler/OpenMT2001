//! Composition of admission, signals, queue/motion dispatch and frame projection.
use super::{
    ControllerSettings, Crossings, Graph, ModelExtent,
    construction::{LANE_BASE, NODE_BASE, ROAD_BASE, RoadNetwork, SEGMENT_BASE},
    projection::{self, ProjectionSettings},
    routes::RoadRoutes,
    word,
};
use crate::{
    resident::AuxiliaryPool,
    traffic::{Admission, RoadVehicles, Signals},
};

/// Original model registration/bounds boundary. Implementations bind the
/// original model definition to the auxiliary and return its numeric Z extents.
pub trait VehicleModels {
    fn bind(&mut self, reference: u32, auxiliary: &mut [u8; 284]) -> Result<(), String>;
    fn extent(&self, auxiliary: &[u8; 284]) -> Result<ModelExtent, String>;
}
pub struct Tick<'a> {
    pub hour: i32,
    pub era: u32,
    pub frequencies: &'a [[i32; 6]; 3],
    pub camera: [f32; 3],
}
pub struct RoadTraffic {
    pub records: Box<[[u8; 208]; 200]>,
    pub auxiliaries: AuxiliaryPool,
    pub admission: Admission,
    pub signals: Signals,
    pub proximity: super::audio::Proximity,
    auxiliary_base: u32,
    controller: ControllerSettings,
    projection: ProjectionSettings,
}
impl RoadTraffic {
    pub fn fresh(
        image: &crate::pe::PeImage<'_>,
        entry_count: usize,
        random: [u32; 3],
        template: &mut [u8; 284],
        auxiliary_base: u32,
        models: &mut impl VehicleModels,
    ) -> Result<Self, String> {
        let mut records: Box<[[u8; 208]; 200]> = vec![[0; 208]; 200]
            .into_boxed_slice()
            .try_into()
            .map_err(|_| "vehicle pool size")?;
        let auxiliaries = crate::traffic::initialize_road_vehicles(
            &mut records,
            template,
            auxiliary_base,
            |reference, auxiliary, _| models.bind(reference, auxiliary),
        )?;
        Ok(Self {
            records,
            auxiliaries,
            admission: Admission {
                random,
                demand: [[0; 7]; 2],
                selected: [0; 2],
                counts: [0; 2],
                entries: i32::try_from(entry_count)
                    .map_err(|_| "entry count exceeds signed range")?,
            },
            signals: Signals::from_v14(image, 0)?,
            proximity: crate::traffic::road::audio::Proximity::default(),
            auxiliary_base,
            controller: ControllerSettings::from_v14(image)?,
            projection: ProjectionSettings::from_v14(image)?,
        })
    }
    /// Original44c210 simulation sequence, including camera proximity ranking.
    /// Host mutex operations remain outside shared simulation logic.
    pub fn advance(
        &mut self,
        network: &mut RoadNetwork,
        routes: &RoadRoutes<'_>,
        tick: Tick<'_>,
        models: &mut impl VehicleModels,
    ) -> Result<(), String> {
        self.admission.entries =
            i32::try_from(network.entry_count).map_err(|_| "entry count exceeds signed range")?;
        self.admission
            .refresh_demands(tick.frequencies, self.signals.counter);
        let mut vehicles = RoadVehicles {
            records: &mut self.records,
            entries: &mut network.entries,
            reference_base: 0x63f9e8,
        };
        let auxiliary_base = self.auxiliary_base;
        self.admission.advance_with_vehicles(
            tick.hour,
            tick.era,
            &mut vehicles,
            |model, reference, _, _| {
                let index =
                    super::record_index(self.auxiliaries.slots(), auxiliary_base, reference)?;
                models.bind(
                    model,
                    self.auxiliaries
                        .get_mut(index)
                        .ok_or("unallocated vehicle auxiliary")?,
                )
            },
        )?;
        self.signals.advance(&mut network.lanes);
        let mut graph = Graph {
            segments: &mut network.segments,
            segment_base: SEGMENT_BASE,
            nodes: &mut network.nodes,
            node_base: NODE_BASE,
        };
        let roads = &network.roads;
        let mut crossings = Crossings {
            lanes: &mut network.lanes,
            lane_base: LANE_BASE,
            roads,
            road_base: ROAD_BASE,
        };
        vehicles.advance_pool(
            &mut graph,
            &mut self.admission.counts,
            |vehicles, index, graph, counts| {
                vehicles.advance_vehicle(
                    index,
                    graph,
                    &mut crossings,
                    &self.controller,
                    counts,
                    |request| routes.choose(request, roads),
                )
            },
            |vehicles, index, graph| {
                let reference = word(&vehicles.records[index], 0xc4);
                let slot =
                    super::record_index(self.auxiliaries.slots(), auxiliary_base, reference)?;
                let extent = models.extent(&self.auxiliaries.slots()[slot])?;
                vehicles
                    .release_from_entry(
                        index,
                        graph,
                        self.auxiliaries
                            .get_mut(slot)
                            .ok_or("unallocated vehicle auxiliary")?,
                        extent,
                    )
                    .map(|_| ())
            },
        )?;
        self.update_proximity(tick.camera)
    }
    pub fn project(&mut self, network: &mut RoadNetwork, fraction: f32) -> Result<(), String> {
        let graph = network.graph();
        let used = self.auxiliaries.used();
        projection::project(
            &self.records,
            &graph,
            &mut self.auxiliaries.slots_mut()[..used],
            self.auxiliary_base,
            fraction,
            &self.projection,
        )
    }
    /// Run after the ordered vehicle tick, using the most recently projected
    /// auxiliary positions, as in original 0044c210.
    pub fn update_proximity(&mut self, camera: [f32; 3]) -> Result<(), String> {
        self.proximity.update(
            &self.records,
            self.auxiliaries.slots(),
            self.auxiliary_base,
            camera,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::put;
    #[test]
    #[ignore = "requires original persistent city traffic checkpoints"]
    fn matches_original_city_traffic() {
        check_city(false);
    }
    #[test]
    #[ignore = "requires original city traffic and original asset bounds"]
    fn matches_original_city_asset_traffic() {
        check_city(true);
    }
    fn check_city(actual_assets: bool) {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let (roads, segment_count) = board.traffic_roads().unwrap();
        let mut categories = [[0; 380]; 52];
        for (i, row) in categories.iter_mut().enumerate() {
            row[4] = board.category_enabled()[i];
            for (axis, value) in board.category_geometry().centers[i].into_iter().enumerate() {
                put(row, 0xe0 + axis * 4, value.to_bits());
            }
        }
        let mut network = RoadNetwork::build(
            &roads,
            segment_count,
            &mut categories,
            52,
            &super::super::construction::ConstructionSettings::from_v14(&image).unwrap(),
        )
        .unwrap();
        let table = std::fs::read(std::env::var("OPENMT2001_ROAD_ROUTE_TABLE").unwrap()).unwrap();
        let routes = RoadRoutes::from_bytes(&table, segment_count as u32).unwrap();
        let data = std::fs::read(
            std::env::var(if actual_assets {
                "OPENMT2001_NATIVE_VEHICLE_CITY_ASSETS"
            } else {
                "OPENMT2001_NATIVE_VEHICLE_CITY"
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(&data[..8], b"MTVCITY1");
        let size = word(&data, 8) as usize;
        let mut at = 16;
        fn rows<const N: usize>(data: &[u8], at: &mut usize, count: usize) -> Vec<[u8; N]> {
            let result = (0..count)
                .map(|i| data[*at + i * N..*at + (i + 1) * N].try_into().unwrap())
                .collect();
            *at += N * count;
            result
        }
        let records = rows::<208>(&data, &mut at, 200)
            .into_boxed_slice()
            .try_into()
            .unwrap();
        assert_eq!(network.segments, rows::<108>(&data, &mut at, 700));
        assert_eq!(network.nodes, rows::<44>(&data, &mut at, 512));
        assert_eq!(network.entries, rows::<52>(&data, &mut at, 64));
        assert_eq!(network.lanes, rows::<32>(&data, &mut at, 63));
        let auxiliaries = AuxiliaryPool::from_slots(rows::<284>(&data, &mut at, 200), 200).unwrap();
        let state: [u32; 9] = std::array::from_fn(|i| word(&data, at + i * 4));
        at += 36;
        assert!(data[at..at + 44].iter().all(|&byte| byte == 0));
        at += 44;
        assert_eq!(at, 16 + size);
        let mut traffic = RoadTraffic {
            records,
            auxiliaries,
            admission: Admission {
                random: state[..3].try_into().unwrap(),
                counts: [state[3], state[4]],
                selected: [state[5] as i32, state[6] as i32],
                demand: [[0; 7]; 2],
                entries: network.entry_count as i32,
            },
            signals: Signals::from_v14(&image, state[7]).unwrap(),
            proximity: crate::traffic::road::audio::Proximity::default(),
            auxiliary_base: 0x10020000,
            controller: ControllerSettings::from_v14(&image).unwrap(),
            projection: ProjectionSettings::from_v14(&image).unwrap(),
        };
        struct Models<'a> {
            tick: u32,
            events: Vec<[u32; 3]>,
            catalog: Option<crate::traffic::road::models::Models<'a>>,
        }
        impl VehicleModels for Models<'_> {
            fn bind(&mut self, model: u32, aux: &mut [u8; 284]) -> Result<(), String> {
                self.events
                    .push([self.tick, model, 0x10020000 + word(aux, 0x38) * 284]);
                if let Some(catalog) = &mut self.catalog {
                    return catalog.bind(model, aux);
                }
                put(aux, 0, model);
                aux[0x44..0xb8].fill(0);
                put(aux, 0x4c, (-2_f32).to_bits());
                put(aux, 0xa0, 2_f32.to_bits());
                Ok(())
            }
            fn extent(&self, aux: &[u8; 284]) -> Result<ModelExtent, String> {
                if let Some(catalog) = &self.catalog {
                    return catalog.extent(aux);
                }
                Ok(ModelExtent { low: -2., high: 2. })
            }
        }
        let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
        let directory = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
        let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let mut models = Models {
            tick: 0,
            events: vec![],
            catalog: if actual_assets {
                Some(
                    crate::traffic::road::models::Models::from_original(&image, &index, &archive)
                        .unwrap(),
                )
            } else {
                None
            },
        };
        let mut completed = 0;
        for _ in 0..word(&data, 12) {
            let end = word(&data, at);
            let events = word(&data, at + 4) as usize;
            at += 8;
            let expected = &data[at..at + size];
            at += size;
            while completed < end {
                let phase = (completed / 500).min(3) as usize;
                models.tick = completed;
                let frequencies = if phase < 3 {
                    [[10; 6], [0; 6], [6; 6]]
                } else {
                    [[0; 6]; 3]
                };
                traffic
                    .advance(
                        &mut network,
                        &routes,
                        Tick {
                            hour: [0, 8, 16, 20][phase],
                            era: [0, 3, 5, 5][phase],
                            frequencies: &frequencies,
                            camera: [0.; 3],
                        },
                        &mut models,
                    )
                    .unwrap_or_else(|e| panic!("tick{completed}: {e}"));
                traffic.project(&mut network, 0.375).unwrap();
                completed += 1;
            }
            let mut actual = Vec::with_capacity(size);
            for row in traffic.records.iter() {
                actual.extend_from_slice(row);
            }
            for row in &network.segments {
                actual.extend_from_slice(row);
            }
            for row in &network.nodes {
                actual.extend_from_slice(row);
            }
            for row in &network.entries {
                actual.extend_from_slice(row);
            }
            for row in &network.lanes {
                actual.extend_from_slice(row);
            }
            for row in traffic.auxiliaries.slots() {
                actual.extend_from_slice(row);
            }
            for value in traffic
                .admission
                .random
                .into_iter()
                .chain(traffic.admission.counts)
                .chain(traffic.admission.selected.map(|v| v as u32))
                .chain([traffic.signals.counter, traffic.signals.phase])
            {
                actual.extend_from_slice(&value.to_le_bytes());
            }
            for value in traffic.proximity.nearest.iter().flatten().copied().chain([
                traffic.proximity.reciprocal_sum.to_bits(),
                traffic.proximity.count as u32,
            ]) {
                actual.extend_from_slice(&value.to_le_bytes());
            }
            assert_eq!(actual.len(), size);
            for (offset, (a, b)) in actual.iter().zip(expected).enumerate() {
                assert_eq!(a, b, "tick{end} byte{offset:#x}");
            }
            let expected_events: Vec<[u32; 3]> = (0..events)
                .map(|_| {
                    let event = std::array::from_fn(|i| word(&data, at + i * 4));
                    at += 12;
                    event
                })
                .collect();
            assert_eq!(models.events, expected_events, "tick{end} bindings");
            models.events.clear();
        }
        assert_eq!(at, data.len());
    }
}
