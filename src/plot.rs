use iced::image;
use plotters::drawing::bitmap_pixel::BGRXPixel;
use plotters::prelude::*;

pub fn graph_bids_asks(
    bids: &[i32],
    asks: &[i32],
) -> Result<image::Handle, Box<dyn std::error::Error>> {
    assert_eq!(bids.len(), asks.len());
    if bids.is_empty() {
        return Ok(image::Handle::from_pixels(0, 0, vec![]));
    }
    let min = bids.iter().chain(asks).copied().min().unwrap();
    let max = bids.iter().chain(asks).copied().max().unwrap();

    const WIDTH: u32 = 200;
    const HEIGHT: u32 = 150;
    let mut buffer = vec![0; WIDTH as usize * HEIGHT as usize * 4];
    let root = BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buffer, (WIDTH, HEIGHT))?
        .into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Top, 5)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 5)
        .build_ranged(0..bids.len(), min..max + 1)?
        .set_secondary_coord(0..bids.len(), min..max + 1);

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    chart.draw_series(LineSeries::new(bids.iter().copied().enumerate(), &BLUE))?;
    chart.draw_secondary_series(LineSeries::new(asks.iter().copied().enumerate(), &RED))?;

    drop(chart);
    drop(root);

    Ok(image::Handle::from_pixels(WIDTH, HEIGHT, buffer))
}

pub fn graph_reserves(reserves: &[u32]) -> Result<image::Handle, Box<dyn std::error::Error>> {
    if reserves.is_empty() {
        return Ok(image::Handle::from_pixels(0, 0, vec![]));
    }
    let min = reserves.iter().copied().min().unwrap();
    let max = reserves.iter().copied().max().unwrap();

    const WIDTH: u32 = 200;
    const HEIGHT: u32 = 150;
    let mut buffer = vec![0; WIDTH as usize * HEIGHT as usize * 4];
    let root = BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buffer, (WIDTH, HEIGHT))?
        .into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Top, 5)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 5)
        .build_ranged(0..reserves.len(), min..max + 1)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    chart.draw_series(LineSeries::new(
        reserves.iter().copied().enumerate(),
        &GREEN,
    ))?;

    drop(chart);
    drop(root);

    Ok(image::Handle::from_pixels(WIDTH, HEIGHT, buffer))
}

pub fn graph_volumes(
    buy_volumes: &[u32],
    sell_volumes: &[u32],
) -> Result<image::Handle, Box<dyn std::error::Error>> {
    assert_eq!(buy_volumes.len(), sell_volumes.len());
    if buy_volumes.is_empty() {
        return Ok(image::Handle::from_pixels(0, 0, vec![]));
    }
    let min = buy_volumes
        .iter()
        .chain(sell_volumes)
        .copied()
        .min()
        .unwrap();
    let max = buy_volumes
        .iter()
        .chain(sell_volumes)
        .copied()
        .max()
        .unwrap();

    const WIDTH: u32 = 200;
    const HEIGHT: u32 = 150;
    let mut buffer = vec![0; WIDTH as usize * HEIGHT as usize * 4];
    let root = BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buffer, (WIDTH, HEIGHT))?
        .into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Top, 5)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 5)
        .build_ranged(0..buy_volumes.len(), min..max + 1)?
        .set_secondary_coord(0..buy_volumes.len(), min..max + 1);

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    chart.draw_series(LineSeries::new(
        buy_volumes.iter().copied().enumerate(),
        &BLUE,
    ))?;
    chart.draw_secondary_series(LineSeries::new(
        sell_volumes.iter().copied().enumerate(),
        &RED,
    ))?;

    drop(chart);
    drop(root);

    Ok(image::Handle::from_pixels(WIDTH, HEIGHT, buffer))
}

pub fn graph_mean_max_age(
    mean_ages: &[u64],
    max_ages: &[u64],
) -> Result<image::Handle, Box<dyn std::error::Error>> {
    assert_eq!(mean_ages.len(), max_ages.len());
    if mean_ages.is_empty() {
        return Ok(image::Handle::from_pixels(0, 0, vec![]));
    }
    let min = mean_ages.iter().chain(max_ages).copied().min().unwrap();
    let max = mean_ages.iter().chain(max_ages).copied().max().unwrap();

    const WIDTH: u32 = 240;
    const HEIGHT: u32 = 200;
    let mut buffer = vec![0; WIDTH as usize * HEIGHT as usize * 4];
    let root = BitMapBackend::<BGRXPixel>::with_buffer_and_format(&mut buffer, (WIDTH, HEIGHT))?
        .into_drawing_area();

    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Top, 5)
        .set_label_area_size(LabelAreaPosition::Left, 80)
        .set_label_area_size(LabelAreaPosition::Bottom, 5)
        .build_ranged(0..mean_ages.len(), min..max + 1)?
        .set_secondary_coord(0..mean_ages.len(), min..max + 1);

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .draw()?;

    chart.draw_series(LineSeries::new(
        mean_ages.iter().copied().enumerate(),
        &CYAN,
    ))?;
    chart.draw_secondary_series(LineSeries::new(
        max_ages.iter().copied().enumerate(),
        &MAGENTA,
    ))?;

    drop(chart);
    drop(root);

    Ok(image::Handle::from_pixels(WIDTH, HEIGHT, buffer))
}
