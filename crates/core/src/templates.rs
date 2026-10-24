//! Starter plans for File › New. Each row is `level|name|duration|predecessors` (row numbers).
use crate::{parse, Project};
use chrono::NaiveDate;

pub struct Template { pub id: &'static str, pub name: &'static str, pub desc: &'static str, pub color: &'static str, rows: &'static str }

pub const TEMPLATES: &[Template] = &[
    Template { id: "engineering", name: "Engineering design", color: "#2F6FCF",
        desc: "Product design from requirements to release: concept, detailed design, prototype, testing and release to manufacturing.",
        rows: "0|Requirements\n1|Kickoff and project plan|2d\n1|Customer and design requirements|5d|2\n1|Requirements signed off|0|3\n\
0|Concept design\n1|Concept options and sketches|6d|4\n1|Concept evaluation and selection|3d|6\n1|Concept design review|0|7\n\
0|Detailed design\n1|3D CAD model|12d|8\n1|Calculations and analysis|8d|10SS+3d\n1|Drawings and bill of materials|6d|10\n1|Design FMEA|3d|11\n1|Critical design review|0|12,13\n\
0|Prototype\n1|Order prototype parts|3d|14\n1|Parts lead time|15d|16\n1|Build prototypes|5d|17\n\
0|Testing and validation\n1|Test plan|3d|14\n1|Functional testing|6d|18,20\n1|Environmental and durability tests|10d|21SS+2d\n1|Design updates from testing|5d|21\n1|Validation report|3d|22,23\n\
0|Release\n1|Final drawing release|2d|24\n1|Manufacturing handover|2d|26\n1|Released to manufacturing|0|27" },
    Template { id: "epc", name: "EPC project", color: "#C2611F",
        desc: "Engineering, procurement and construction of a plant unit, through commissioning and handover, with long-lead equipment.",
        rows: "0|Engineering\n1|Basic engineering package review|10d\n1|Process design and P&IDs|20d|2\n1|Mechanical and piping design|30d|3SS+10d\n1|Electrical and instrumentation design|25d|3SS+15d\n1|Issued for construction drawings|0|4,5\n\
0|Procurement\n1|Long-lead equipment orders|10d|3\n1|Long-lead equipment manufacture|80d|8\n1|Bulk materials purchase|15d|4\n1|Equipment delivered to site|0|9\n\
0|Construction\n1|Site mobilisation|5d|6\n1|Civil works and foundations|30d|13\n1|Equipment installation|20d|14,11\n1|Piping installation|35d|15SS+10d,10\n1|Electrical and instrumentation installation|30d|16SS+10d\n1|Mechanical completion|0|16,17\n\
0|Commissioning and handover\n1|Pre-commissioning checks|10d|18\n1|Commissioning|15d|20\n1|Performance test run|5d|21\n1|Handover to client|0|22" },
    Template { id: "software", name: "Software development", color: "#2E9D6B",
        desc: "A product release in sprints: discovery, design, three build sprints, testing, launch and support.",
        rows: "0|Discovery\n1|Kickoff|1d\n1|Requirements and user stories|5d|2\n1|Architecture and technical design|5d|3\n1|Discovery complete|0|4\n\
0|Design\n1|UX wireframes|5d|3\n1|UI design|5d|7\n1|Design sign-off|0|8,4\n\
0|Build\n1|Sprint 1|10d|9\n1|Sprint 2|10d|11\n1|Sprint 3|10d|12\n1|Feature complete|0|13\n\
0|Test and launch\n1|System and integration testing|8d|14\n1|User acceptance testing|5d|16\n1|Release preparation|3d|17SS+2d\n1|Go live|0|17,18\n\
0|Support\n1|Hypercare|10d|19\n1|Project close|0|21" },
    Template { id: "building", name: "Building construction", color: "#8A6D1F",
        desc: "A commercial building from permits to handover: foundations, frame, envelope, services, fit-out and inspections.",
        rows: "0|Pre-construction\n1|Design approval and permits|20d\n1|Contractor tendering|15d|2SS+10d\n1|Contract award|0|2,3\n\
0|Substructure\n1|Site setup|5d|4\n1|Excavation|8d|6\n1|Foundations|12d|7\n1|Ground floor slab|6d|8\n\
0|Superstructure\n1|Structural frame|25d|9\n1|Floors and roof structure|15d|11SS+10d\n1|Structure complete|0|11,12\n\
0|Envelope\n1|External walls|20d|13\n1|Roofing|10d|12\n1|Windows and doors|10d|15SS+10d\n1|Building watertight|0|16,17\n\
0|Services and fit-out\n1|Mechanical and plumbing|25d|18\n1|Electrical|25d|18\n1|Internal finishes|30d|20SS+10d,21SS+10d\n1|External works|15d|18\n\
0|Handover\n1|Inspections and snagging|10d|22,23\n1|Certificates and as-built documents|5d|25\n1|Practical completion|0|26" },
    Template { id: "research", name: "Research project", color: "#7A4FC4",
        desc: "A funded study: proposal and approvals, literature review, data collection, analysis and publication.",
        rows: "0|Proposal\n1|Literature scan|10d\n1|Write proposal|10d|2\n1|Ethics and funding approval|20d|3\n1|Approved|0|4\n\
0|Literature review\n1|Systematic search|10d|5\n1|Review and synthesis|15d|7\n\
0|Data collection\n1|Instrument design|10d|5\n1|Pilot study|5d|10\n1|Main data collection|40d|11\n1|Data collected|0|12\n\
0|Analysis\n1|Data cleaning|8d|13\n1|Statistical analysis|15d|15\n1|Interpretation|10d|16,8\n\
0|Publication\n1|Draft paper|15d|17\n1|Internal review|5d|19\n1|Submit for publication|0|20" },
];

pub fn build(t: &Template, start: NaiveDate) -> Project {
    let mut p = Project::new(t.name, start);
    let rows: Vec<Vec<&str>> = t.rows.lines().map(|l| l.split('|').collect()).collect();
    for r in &rows {
        let task = p.new_task(r[1], r[0].parse().unwrap_or(0), r.get(2).map_or(0, |d| parse::dur(d).unwrap_or(0)));
        p.tasks.push(task);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (task, r) in p.tasks.iter_mut().zip(&rows) {
        task.preds = parse::preds(r.get(3).unwrap_or(&""), &ids).expect("template links are valid");
    }
    p
}
