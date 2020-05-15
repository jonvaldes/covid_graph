use chrono::prelude::*;
use anyhow::Result;

pub struct GraphPalette;

impl plotters::prelude::Palette for GraphPalette {
    const COLORS: &'static [(u8, u8, u8)] = &[
        (230, 25, 75),
        (60, 180, 75),
        (255, 225, 25),
        (0, 130, 200),
        (245, 130, 48),
        (145, 30, 180),
        (70, 240, 240),
        (240, 50, 230),
        (210, 245, 60),
        (250, 190, 190),
        (0, 128, 128),
        (230, 190, 255),
        (170, 110, 40),
        (155, 250, 200),
        (128, 0, 0),
        (170, 255, 195),
        (128, 128, 0),
        (255, 215, 180),
        (0, 0, 128),
        (128, 128, 128),
        (0, 0, 0),
    ];
}

#[derive(Debug)]
struct DataPoint {
    name: String,
    region: String,
    last_update: chrono::DateTime<Utc>,
    confirmed: u32,
    deaths: u32,
    recovered: u32,
}

fn get_covid_data() -> Result<Vec<DataPoint>> {

    let url = "https://nssac.bii.virginia.edu/covid-19/dashboard/data/nssac-ncov-data-country-state.zip";

    let bytes = reqwest::blocking::get(url)?.bytes()?;

    let byte_slice = bytes.slice(..);
    let mut data_cursor = std::io::Cursor::new(byte_slice);
    let mut zip_data = zip::read::ZipArchive::new(&mut data_cursor)?;

    let file_count = zip_data.len();

    let mut result = vec![];

    for index in 0..file_count {
        let file = zip_data.by_index(index)?;
        let filename = String::from(file.name());
        if filename.contains("README") {
            continue;
        }

        let reader = std::io::BufReader::new(file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .flexible(true)
            .terminator(csv::Terminator::Any('\n' as u8))
            .from_reader(reader);

        let mut parse_file = || -> Result<()> {
            for record in csv_reader.records() {
                let record = record?;
                if record.len() != 6 {
                    println!("Record of wrong length: {:?}", record);
                    continue;
                }
                let data_point = DataPoint {
                    name: String::from(&record[0]),
                    region: String::from(&record[1]),
                    last_update: Utc.datetime_from_str(&record[2], "%Y-%m-%d %H:%M:%S")?,
                    confirmed: record[3].parse()?,
                    deaths: record[4].parse()?,
                    recovered: record[5].parse()?,
                };

                result.push(data_point);
            }
            Ok(())
        };

        if let Err(err) = parse_file() {
            println!("Error parsing file {}: {}", filename, err);
        }
    }

    Ok(result)
}


#[derive(Clone)]
struct EvolutionPoint {
    update_time: chrono::DateTime<Utc>,
    confirmed: u32,
    deaths: u32,
    recovered: u32,
}

#[derive(Clone)]
struct RegionData {
    name: String,
    region: String,
    evolution: Vec<EvolutionPoint>,
}

fn get_per_country_data() -> Result<Vec<RegionData>> {

    use std::collections::HashMap;
    let mut hash: HashMap<String, RegionData> = HashMap::new();

    for data_point in get_covid_data()? {
        let evolution_point = EvolutionPoint {
            update_time: data_point.last_update,
            confirmed: data_point.confirmed,
            deaths: data_point.deaths,
            recovered: data_point.recovered,
        };

        if let Some(value) = hash.get_mut(&data_point.name) {
            value.evolution.push(evolution_point);
        } else {
            hash.insert(
                data_point.name.clone(),
                RegionData {
                    name: data_point.name,
                    region: data_point.region,
                    evolution: vec![evolution_point],
                },
            );
        }
    }

    Ok(hash.values().cloned().collect())
}

fn population_map() -> std::collections::HashMap<String, u32> {

    macro_rules! map(
    { $($key:expr => $value:expr)+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert(String::from($key), $value);
            )+
            m
        }
     };
    );

    map!{
        "Norway" => 5_295_619
        "Sweden" => 10_090_825
        "Spain" => 46_752_408
        "Italy" => 60_461_826
        "France" => 65_255_227
        "Germany" => 83_749_987
        "Belgium" => 11_583_221
        "Finland" => 5_539_631
        "Austria" => 8_999_865
        "Netherlands" => 17_130_073
        "Switzerland" =>  8_646_561
        "United Kingdom" => 67_886_011
        "Ireland" => 4_937_786
        "Denmark" => 5_775_666
    }
}

fn draw_evolution_graph(regions: &Vec<RegionData>) -> Result<()> {

    use plotters::prelude::*;

    let root_area = BitMapBackend::new("total.png", (1024, 1024)).into_drawing_area();
    root_area.fill(&WHITE)?;

    let (upper, lower) = root_area.split_vertically(512);

    macro_rules! setup_chart {
        ($name: expr, $area: ident, $vert_max: expr) => {

            {
            let mut cc = ChartBuilder::on(&$area)
                .margin(5)
                .set_all_label_area_size(50)
                .caption($name, ("sans-serif", 40).into_font())
                .build_ranged(0.0..(24*5*30) as f32, 0.0 ..$vert_max as f32).unwrap();

            cc.configure_mesh()
                .x_labels(20)
                .y_labels(10)
                .disable_mesh()
                .x_label_formatter(&|v| format!("{:.0}", v/24.0))
                .y_label_formatter(&|v| format!("{:.1}", v))
                .draw().unwrap();
            cc
            }
        }
    }
        
    let population_map = population_map();

    let countries = population_map.keys().cloned().collect::<Vec<String>>();
    // Draw total death count graph
    {
        let mut cc = setup_chart!("Total Deaths", upper, 40_000.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.name))
            .enumerate()
        {
            let t0 = Utc.ymd(2020, 1, 22).and_hms(0, 0, 0);

            cc.draw_series(LineSeries::new(
                country.evolution.iter().map(|point| {
                    (
                        point.update_time.signed_duration_since(t0).num_hours() as f32,
                        point.deaths as f32,
                    )
                }),
                &GraphPalette::pick(index),
            ))?
                .label(country.name.clone())
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }
        cc.configure_series_labels().border_style(&BLACK).draw()?;
    }

    // Draw deaths per 100K people graph
    {
        let mut cc = setup_chart!("Deaths per 100K", lower, 80.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.name))
            .enumerate()
        {
            let t0 = Utc.ymd(2020, 1, 22).and_hms(0, 0, 0);

            let population_over100k = population_map.get(&country.name).cloned().unwrap() as f32 /
                100_000 as f32;

            cc.draw_series(LineSeries::new(
                country.evolution.iter().map(|point| {
                    (
                        point.update_time.signed_duration_since(t0).num_hours() as f32,
                        point.deaths as f32 / population_over100k,
                    )
                }),
                &GraphPalette::pick(index),
            ))?
                .label(country.name.clone())
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }
        cc.configure_series_labels().border_style(&BLACK).draw()?;
    }

    Ok(())
}


fn main() -> Result<()> {
    let country_data = get_per_country_data()?;

    draw_evolution_graph(&country_data)?;

    Ok(())
}
