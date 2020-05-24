// TODO - Check this data source: https://opendata.ecdc.europa.eu/covid19/casedistribution/json/
use std::collections::HashMap;

use serde::Deserialize;
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


#[derive(Debug, Clone)]
struct Record {
    day: i32, // Days since January 1st 2020
    cases: i32,
    deaths: i32,
}

#[derive(Debug, Clone)]
struct CountryData {
    country_name: String,
    records: Vec<Record>,
    population_2018: i32,
}

fn get_covid_data_json() -> Result<Vec<CountryData>> {

    #[derive(Deserialize)]
    struct JsonData {
        records: Vec<JsonDataPoint>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct JsonDataPoint {
        date_rep: String,
        day: String,
        month: String,
        year: String,
        cases: String,
        deaths: String,
        countries_and_territories: String,
        geo_id: String,
        countryterritory_code: String,
        pop_data_2018: String,
        continent_exp: String,
    }



    let url = "https://opendata.ecdc.europa.eu/covid19/casedistribution/json/";
    let json_data : JsonData = reqwest::blocking::get(url)?.json()?;

    let mut country_map : HashMap<String, CountryData> = HashMap::new();

    let first_day_of_month = [0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 336];

    for json_record in json_data.records {

        if json_record.year != "2020" {
            continue;
        }

        let record = Record {
            day: json_record.day.parse::<i32>()? + first_day_of_month[json_record.month.parse::<usize>()?],
            cases: json_record.cases.parse()?,
            deaths: json_record.deaths.parse()?,
        };


        if let Some(value) = country_map.get_mut(&json_record.countries_and_territories) {
            value.records.push(record);
        } else {
            country_map.insert(json_record.countries_and_territories.clone(), 
                CountryData{
                    country_name: json_record.countries_and_territories.replace("_"," "),
                    records: vec!(record),
                    population_2018: json_record.pop_data_2018.parse().unwrap_or(0),
                });
        }
    }

    Ok(country_map.values().cloned().collect())
}

fn averaged_daily_deaths(records: &[Record], current_day: usize, avg_days: usize) -> f32 {

    let total_counts : i32 = records.iter().skip(current_day - avg_days).take(avg_days).map(|record| record.deaths).sum();
    total_counts as f32 / avg_days as f32
}

fn draw_evolution_graph(regions: &Vec<CountryData>) -> Result<()> {

    use plotters::prelude::*;

    let root_area = BitMapBackend::new("total.png", (1920, 1024)).into_drawing_area();
    root_area.fill(&WHITE)?;

    let (upper, lower) = root_area.split_vertically(512);
    let (top_left, top_right) = upper.split_horizontally(960);
    let (bottom_left, bottom_right) = lower.split_horizontally(960);

    macro_rules! setup_chart {
        ($name: expr, $area: ident, $vert_max: expr) => {

            {
            let mut cc = ChartBuilder::on(&$area)
                .margin(5)
                .set_all_label_area_size(50)
                .caption($name, ("sans-serif", 40).into_font())
                .build_ranged(
                    (30f32 * 3f32)..(30f32*6f32),
                    0f32..$vert_max as f32).unwrap();

            cc.configure_mesh()
                .x_labels(20)
                .y_labels(10)
                .disable_mesh()
                .x_label_formatter(&|v| format!("{:.0}", v))
                .y_label_formatter(&|v| format!("{:.1}", v))
                .draw().unwrap();
            cc
            }
        }
    }
    let average_days = 7;
        
    let countries = &["Spain", "Sweden", "Belgium", "United Kingdom", "Germany", "Brazil", "United States of America"];
    // Draw total death count graph
    {
        let mut cc = setup_chart!("Total Deaths", top_left, 100_000.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.country_name.as_str()))
            .enumerate()
        {

            let (accum_deaths, death_sum) = {
                let mut running_sum = 0;
                let mut deaths = vec!();
                for record in country.records.iter().rev() {
                    running_sum += record.deaths;
                    deaths.push( (record.day as f32, running_sum as f32));
                }
                (deaths, running_sum)
            };

            cc.draw_series(LineSeries::new(accum_deaths, &GraphPalette::pick(index)))?
                .label(format!("{} - Current deaths: {}", country.country_name, death_sum))
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }
        cc.configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK).draw()?;
    }
    
    // Draw daily deaths
    {
        let mut cc = setup_chart!(format!("Total daily Deaths Averaged over {} days", average_days), top_right, 3_000.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.country_name.as_str()))
            .enumerate()
        {

            cc.draw_series(LineSeries::new(
                country.records.iter().enumerate().skip(average_days).map(|(index,record)| {
                    (record.day as f32, 
                    averaged_daily_deaths(&country.records, index, average_days))
                }),
                &GraphPalette::pick(index)))?
                .label(country.country_name.clone())
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }
        cc.configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK).draw()?;
    }

    // Draw deaths per 100K people graph
    {
        let mut cc = setup_chart!("Deaths per 100K", bottom_left, 90.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.country_name.as_str()))
            .enumerate()
        {

            let population_over100k = country.population_2018 as f32 / 100_000 as f32;

            let accum_deaths = {
                let mut running_sum = 0;
                let mut deaths = vec!();
                for record in country.records.iter().rev() {
                    running_sum += record.deaths;
                    deaths.push( (record.day as f32, running_sum as f32 / population_over100k) );
                }
                deaths
            };

            cc.draw_series(LineSeries::new( accum_deaths, &GraphPalette::pick(index)))?
                .label(country.country_name.clone())
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }
        cc.configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK).draw()?;
    }
    
    {
        let mut cc = setup_chart!(format!("Daily Deaths per 100K, Averaged over {} days", average_days), bottom_right, 4.0);

        for (index, country) in regions
            .iter()
            .filter(|x| countries.contains(&x.country_name.as_str()))
            .enumerate()
        {
            
            let population_over100k = country.population_2018 as f32 / 100_000 as f32;

            cc.draw_series(LineSeries::new(
                country.records.iter().enumerate().skip(average_days).map(|(index, point)| {

                    (
                        point.day as f32,
                        averaged_daily_deaths(&country.records, index, average_days) / population_over100k,
                        //point.deaths as f32 / population_over100k,
                    )
                }),
                &GraphPalette::pick(index),
            ))?
                .label(country.country_name.clone())
                .legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &GraphPalette::pick(index))
                });
        }

        cc.configure_series_labels()
            .position(plotters::chart::SeriesLabelPosition::UpperLeft)
            .border_style(&BLACK).draw()?;
    }

    Ok(())
}


fn main() -> Result<()> {
    let countries = get_covid_data_json()?;

    draw_evolution_graph(&countries)?;

    Ok(())
}
