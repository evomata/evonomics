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
        .set_label_area_size(LabelAreaPosition::Left, 30)
        .build_ranged(0..bids.len(), min..max)?
        .set_secondary_coord(0..bids.len(), min..max);

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

pub fn graph_reserve(reserves: &[u32]) -> Result<image::Handle, Box<dyn std::error::Error>> {
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
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .build_ranged(0..reserves.len(), min..max)?;

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
