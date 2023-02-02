#![allow(dead_code)]

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
// use cjval;

use indexmap::IndexSet;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rust_decimal::Decimal;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Clone)]
struct Vertex([Decimal; 3]);
// #[serde(with = "rust_decimal::serde::float")]
//     coords: [Decimalk; 3]
// }

// impl serde::Serialize for Vertex {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//         where
//             S: serde::Serializer,
//     {
//         // use num::ToPrimitive;
//         let mut tuple = serializer.serialize_tuple(3)?;
//         tuple.serialize_element(&self.0[0].to_f64())?;
//         tuple.end()
//     }
// }

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Clone)]
struct TriangleIndices([usize; 3]);
impl TriangleIndices {
    pub(crate) fn from_vec(iter: Vec<usize>) -> TriangleIndices {
        let arr: [usize; 3] = iter.as_slice().try_into().unwrap();
        TriangleIndices(arr)
    }
}

#[derive(Serialize, Deserialize)]
enum CityObjectType {
    // All CityGML 2.0 types
    CityFurniture,
    CityObjectGroup,
    GenericCityObject,
    LandUse,
    PlantCover,
    SolitaryVegetationObject,
    WaterBody,
    AuxiliaryTrafficArea,
    Railway,
    Road,
    Track,
    TrafficArea,
    Tunnel,
    Bridge,
    BridgeConstructionElement,
    BridgeFurniture,
    BridgeInstallation,
    BridgePart,
    BridgeRoom,
    BridgeRoofSurface,
    BridgeWallSurface,
    Building,
    BuildingInstallation,
    TINRelief,
}

#[derive(Serialize, Deserialize)]
enum CityObjectGeometryType {
    // All CityGML 2.0 geometry types
    CompositeSolid,
    MultiSolid,
    MultiCurve,
    MultiPoint,
    MultiSolidCoverage,
    MultiSurfaceCoverage,
    MultiCurveCoverage,
    MultiPointCoverage,
    CompositeCurve,
    CompositePoint,
    CompositeSurface,
    MultiSurface,
}

#[derive(Clone, Copy)]
enum CityObjectGeometryLOD {
    One,
    Two,
    Three,
}

impl Serialize for CityObjectGeometryLOD {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CityObjectGeometryLOD::One => serializer.serialize_str("1"),
            CityObjectGeometryLOD::Two => serializer.serialize_str("2"),
            CityObjectGeometryLOD::Three => serializer.serialize_str("3"),
        }
    }
}

impl<'de> Deserialize<'de> for CityObjectGeometryLOD {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "1" => Ok(CityObjectGeometryLOD::One),
            "2" => Ok(CityObjectGeometryLOD::Two),
            "3" => Ok(CityObjectGeometryLOD::Three),
            _ => Err(serde::de::Error::custom("Invalid CityObjectGeometryLOD")),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Boundary(Vec<TriangleIndices>);

#[derive(Serialize, Deserialize)]
struct CityObjectGeometry {
    r#type: CityObjectGeometryType,
    lod: CityObjectGeometryLOD,
    boundaries: Vec<Boundary>,
}

#[derive(Serialize, Deserialize)]
struct CityObject {
    r#type: CityObjectType,
    geometry: Vec<CityObjectGeometry>,
}

impl CityObject {
    pub(crate) fn new(r#type: CityObjectType) -> CityObject {
        CityObject {
            r#type,
            geometry: vec![],
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CityModelTransform {
    scale: [Decimal; 3],
    translate: [Decimal; 3],
}

#[derive(Deserialize)]
struct CityModel {
    #[serde(rename = "CityObjects")]
    objects: HashMap<String, CityObject>,
    transform: CityModelTransform,
    vertices: IndexSet<Vertex>,
}

impl Serialize for CityModel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut obj = serializer.serialize_map(None)?;
        obj.serialize_entry("type", "CityJSON")?;
        obj.serialize_entry("version", "1.1")?;
        obj.serialize_entry("transform", &self.transform)?;
        obj.serialize_entry("CityObjects", &self.objects)?;
        obj.serialize_entry("vertices", &self.vertices)?;
        obj.end()
    }
}

fn parse(s: &str) -> Vec<Vertex> {
    s.split_whitespace()
        .map(|num| num.parse().unwrap())
        .collect::<Vec<Decimal>>()
        .chunks(3)
        .map(|chunk| Vertex([chunk[0], chunk[1], chunk[2]]))
        .collect()
}

impl CityModel {
    fn from_file(path: &Path) -> Result<CityModel, quick_xml::Error> {
        let mut reader = Reader::from_file(path)?;
        let mut buf = Vec::new();
        let mut collect = false;
        let mut triangles: Vec<TriangleIndices> = Vec::new();
        let mut model = CityModel::empty();
        let mut current_cityobject: Option<CityObject> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        // b"gml:TriangulatedSurface" => println!("attributes values: {:?}",
                        //                     e.attributes().map(|a| a.unwrap().value)
                        //                     .collect::<Vec<_>>()),
                        b"gml:posList" => collect = true,
                        b"dem:TINRelief" => {
                            current_cityobject = Some(CityObject::new(CityObjectType::TINRelief))
                        }
                        _ => (),
                    }
                }

                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"gml:posList" => collect = false,
                    b"dem:TINRelief" => {
                        if let Some(mut co) = current_cityobject {
                            co.geometry = vec![CityObjectGeometry {
                                r#type: CityObjectGeometryType::CompositeSurface,
                                lod: CityObjectGeometryLOD::Two,
                                boundaries: triangles
                                    .into_iter()
                                    .map(|t| Boundary(vec![t]))
                                    .collect(),
                            }];
                            model.objects.insert("ground".to_string(), co);
                        }
                        current_cityobject = None;
                        triangles = Vec::new()
                    }
                    _ => (),
                },

                Ok(Event::Text(e)) => {
                    if collect {
                        // dbg!(parse(&e.unescape().unwrap().into_owned()));
                        let my_vertices = parse(&e.unescape().unwrap().into_owned());
                        let triangle = TriangleIndices::from_vec(
                            my_vertices
                                .into_iter()
                                .take(3)
                                .map(|vertex| {
                                    let (index, _) = model.vertices.insert_full(vertex);
                                    index
                                })
                                .collect(),
                        );
                        triangles.push(triangle);
                    }
                    ()
                }

                Ok(Event::Eof) => break,

                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),

                _ => (),
            }
            buf.clear();
        }
        Ok(model)
    }

    pub(crate) fn empty() -> CityModel {
        CityModel {
            objects: HashMap::new(),
            transform: CityModelTransform {
                scale: [Decimal::ONE, Decimal::ONE, Decimal::ONE],
                translate: [Decimal::ZERO, Decimal::ZERO, Decimal::ZERO],
            },
            vertices: IndexSet::new(),
        }
    }
}

fn main() {
    pretty_env_logger::formatted_timed_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Read input from the program arguments
    let input = std::env::args().nth(1).expect("Input file");
    let output_path = std::env::args().nth(2).expect("Output file");

    // Print size of input, in MB
    let input_size = std::fs::metadata(&input).unwrap().len() as f64 / 1024.0 / 1024.0;
    info!("Loading CityGML: {} MB", input_size);
    let model = CityModel::from_file(input.as_ref()).expect("Valid CityGML");
    info!(
        "Loaded CityModel with {} objects and {} vertices",
        model.objects.len(),
        model.vertices.len()
    );
    info!("Deserializing to CityJSON");
    let s = serde_json::to_string(&model).unwrap();
    info!("Size of CityJSON: {} MB", s.len() as f64 / 1024.0 / 1024.0);
    let mut output = File::create(&output_path).unwrap();
    output.write_all(s.as_bytes()).unwrap();
    info!("Wrote CityJSON to {}", output_path);

    // Load the CityJSON again using serde_json
    info!("Loading CityJSON again");
    let s2 = std::fs::read_to_string(&output_path).unwrap();
    let model2: CityModel = serde_json::from_str(&s2).unwrap();
    info!(
        "Loaded CityModel with {} objects and {} vertices",
        model2.objects.len(),
        model2.vertices.len()
    );

    // let s1 = std::fs::read_to_string("data.json")
    //         .expect("Couldn't read CityJSON file");
    // let v = cjval::CJValidator::from_str(&s1);
    // if v.is_valid() {
    //     info!("CityJSON validation successful");
    // } else {
    //     warn!("Validation failed")
    // }
}
