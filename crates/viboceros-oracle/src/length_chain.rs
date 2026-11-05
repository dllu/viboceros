//! Independent Length/SelChain input replay through the shared selection and command APIs.
use super::*;
use crate::chain_selection::Fixture;
use viboceros_command::chain_selection::{Session, Target, defaults, parse_options};

struct Getter {
    doc: Document,
    registry: CommandRegistry,
    chain: Option<Session>,
    options: viboceros_geometry::CurveChainOptions,
    active: bool,
    units_pending: bool,
    centimeters: bool,
    length_mm: Option<f64>,
}
impl Getter {
    fn phase(&self) -> &'static str {
        if !self.active {
            "idle"
        } else if let Some(s) = &self.chain {
            if s.choosing_branch() {
                "branch"
            } else if s.has_seed() {
                "connected"
            } else {
                "seed"
            }
        } else if self.units_pending {
            "units"
        } else {
            "parent"
        }
    }
    fn snapshot(&self, ids: &[ObjectId]) -> Result<Value, ProbeError> {
        let mut components = self
            .registry
            .component_selection()
            .checked_picks(&self.doc)
            .map_err(ProbeError::FixtureInvariant)?
            .into_iter()
            .map(|p| (ids.iter().position(|&id| id == p.object).unwrap(), p.index))
            .collect::<Vec<_>>();
        components.sort_unstable();
        components.dedup();
        Ok(
            json!({"whole":ids.iter().enumerate().filter_map(|(i,&id)|self.doc.is_selected(id).then_some(i)).collect::<Vec<_>>(),"objects":self.doc.objects().len(),"components":components.into_iter().map(|(object,index)|json!({"object":object,"index":index,"kind":"BrepEdge"})).collect::<Vec<_>>(),"phase":self.phase()}),
        )
    }
    fn enter(&mut self) -> Result<(), ProbeError> {
        if let Some(s) = &mut self.chain {
            let done = s.enter(&self.doc)?;
            s.apply(&mut self.doc, self.registry.component_selection())?;
            if done {
                self.chain = None;
            }
        } else if self.active {
            let command = if self.centimeters {
                "Length Units=Centimeters"
            } else {
                "Length"
            };
            let report =
                self.registry
                    .execute_postselected(&mut self.doc, command, Default::default())?;
            let value = report
                .split_once("total length ")
                .and_then(|(_, s)| s.split_whitespace().next())
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or(ProbeError::FixtureInvariant(
                    "numeric Length report required",
                ))?;
            self.length_mm = Some(value * if self.centimeters { 10. } else { 1. });
            self.active = false;
        }
        Ok(())
    }
    fn cancel(&mut self) {
        self.chain = None;
        self.active = false;
        self.units_pending = false;
        self.doc.clear_selection();
        self.registry.component_selection().clear();
    }
    fn key(&mut self, key: &str) -> Result<(), ProbeError> {
        match key {
            "Escape" => self.cancel(),
            "Enter" => self.enter()?,
            "SelChain" if self.active && self.chain.is_none() => {
                self.chain = Some(Session::new(
                    &self.doc,
                    self.registry.component_selection(),
                    self.options,
                )?);
            }
            "Units" if self.active && self.chain.is_none() => self.units_pending = true,
            "Centimeter" | "Centimeters" if self.units_pending => {
                self.centimeters = true;
                self.units_pending = false;
            }
            // Viboceros also accepts the existing Units= scripting convenience.
            "Units=Centimeters" if self.active && self.chain.is_none() => self.centimeters = true,
            "Undo" => self
                .chain
                .as_mut()
                .ok_or(ProbeError::FixtureInvariant("chain getter required"))?
                .undo(&self.doc)?,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "owned Length chain input required",
                ));
            }
        }
        if let Some(s) = &self.chain {
            s.apply(&mut self.doc, self.registry.component_selection())?;
        }
        Ok(())
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !f.pre.is_empty() || f.steps.len() > 24 || !matches!(f.finish.as_str(), "Enter" | "Cancel") {
        return Err(ProbeError::FixtureInvariant(
            "owned Length chain workflow required",
        ));
    }
    measure_probe(iterations, || {
        let mut doc = Document::new(tolerance);
        doc.set_units(viboceros_geometry::LengthUnitSystem::Millimeters, false)?;
        let ids = chain_selection::source(&f.scene, tolerance)?
            .into_iter()
            .map(|g| doc.add_geometry(g))
            .collect::<Result<Vec<_>, _>>()?;
        doc.clear_history()?;
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        let mut options = defaults(&doc);
        for (key, value) in &f.options {
            let name = match key.as_str() {
                "continuity" => "ChainContinuity",
                "direction" => "Direction",
                "gap" => "GapTolerance",
                "angle" => "AngleTolerance",
                _ => return Err(ProbeError::FixtureInvariant("owned chain option required")),
            };
            options = parse_options(&format!("{name}={value}"), options, tolerance)?;
        }
        let mut getter = Getter {
            doc,
            registry,
            chain: None,
            options,
            active: false,
            units_pending: false,
            centimeters: false,
            length_mm: None,
        };
        let before = getter.snapshot(&ids)?;
        getter.active = true;
        getter.chain = Some(Session::new(
            &getter.doc,
            getter.registry.component_selection(),
            options,
        )?);
        let mut trace = Vec::new();
        for step in &f.steps {
            if step.kind == "click" {
                let source = step
                    .source
                    .or(f.seed)
                    .and_then(|i| ids.get(i).copied())
                    .ok_or(ProbeError::FixtureInvariant("owned Length seed required"))?;
                let target = if getter
                    .doc
                    .object(source)
                    .unwrap()
                    .geometry()
                    .curve_ref()
                    .is_some()
                {
                    Target::Curve(source)
                } else {
                    Target::Edge(source, 0)
                };
                getter
                    .chain
                    .as_mut()
                    .ok_or(ProbeError::FixtureInvariant("chain seed phase required"))?
                    .pick_input(&getter.doc, target)?;
                getter
                    .chain
                    .as_ref()
                    .unwrap()
                    .apply(&mut getter.doc, getter.registry.component_selection())?;
            } else if step.kind == "key" {
                getter.key(
                    step.value
                        .as_deref()
                        .ok_or(ProbeError::FixtureInvariant("chain key required"))?,
                )?;
            } else {
                return Err(ProbeError::FixtureInvariant("owned Length input required"));
            }
            trace.push(getter.snapshot(&ids)?);
        }
        if f.finish == "Cancel" {
            getter.cancel();
        } else {
            getter.enter()?;
        }
        if getter.active {
            return Err(ProbeError::FixtureInvariant(
                "finish nested chain and parent Length getter",
            ));
        }
        let after = getter.snapshot(&ids)?;
        getter.registry.execute(&mut getter.doc, "Redo")?;
        let redo = getter.snapshot(&ids)?;
        getter.registry.execute(&mut getter.doc, "Undo")?;
        let undo = getter.snapshot(&ids)?;
        Ok(
            json!({"before":before,"trace":trace,"after":after,"redo":redo,"undo":undo,"success":getter.length_mm.is_some(),"length_mm":getter.length_mm,"source_unchanged":ids.iter().zip(&original).all(|(&id,o)|getter.doc.object(id)==Some(o))}),
        )
    })
}
