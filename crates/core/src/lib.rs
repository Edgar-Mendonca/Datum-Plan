//! Datum Plan core: everything except the window. No UI or OS code lives here.
pub mod cal;
pub mod doc;
pub mod model;
pub mod parse;
pub mod sched;
pub mod view;

pub use model::*;

use chrono::NaiveDate;

/// A starter plan: a centrifugal compressor package from kick-off to shipment.
pub fn sample(start: NaiveDate) -> Project {
    const ROWS: &[(u8, &str, i32, &str)] = &[
        (0, "Compressor Package", 0, ""),
        (1, "Engineering", 0, ""),
        (2, "Kick-off meeting", 0, ""),
        (2, "Process design and datasheets", 10, "3"),
        (2, "GA drawing and P&ID approval", 15, "4"),
        (2, "Rotor dynamics study", 8, "4SS+5d"),
        (1, "Procurement", 0, ""),
        (2, "Order long-lead items", 5, "5"),
        (2, "Casing and rotor forgings", 40, "8"),
        (2, "Bought-out items", 25, "8"),
        (1, "Manufacturing", 0, ""),
        (2, "Machining and rotor assembly", 30, "9, 6"),
        (2, "Skid fabrication", 20, "5"),
        (2, "Package assembly", 15, "12, 13"),
        (1, "Testing and Shipment", 0, ""),
        (2, "Mechanical running test", 5, "14"),
        (2, "Performance test", 5, "16"),
        (2, "Preservation and packing", 4, "17"),
        (2, "Ready for shipment", 0, "18"),
    ];
    let mut p = Project::new("Compressor Package", start);
    for &(level, name, dur, _) in ROWS {
        let t = p.new_task(name, level, dur);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(.., preds)) in p.tasks.iter_mut().zip(ROWS) {
        t.preds = parse::preds(preds, &ids).expect("sample links are valid");
    }
    p.tasks[3].pct = 100;
    p.tasks[4].pct = 40;
    p
}

#[cfg(test)]
mod tests;
