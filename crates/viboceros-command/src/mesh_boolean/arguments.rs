use super::*;

const USAGE: &str = "MeshBooleanUnion [Tolerance=Auto|Document|Zero]; MeshBooleanIntersection|MeshBooleanDifference [Tolerance=Auto|Document|Zero] [DeleteInput=Yes|No] FirstSet=ids SecondSet=ids";
#[derive(Clone, Copy)]
pub(super) struct Options {
    pub tolerance: &'static str,
    pub delete_cutters: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            tolerance: "Auto",
            delete_cutters: true,
        }
    }
}
impl Options {
    pub fn geometry_tolerance(self, document: Tolerance) -> Tolerance {
        // Public command contract: Auto scales model tolerance by the mesh
        // intersection coefficient; the owned native capture records 0.0001.
        // Zero uses ten ON_ZERO_TOLERANCE units. See the command evidence page.
        let absolute = match self.tolerance {
            "Auto" => (document.absolute() * 1e-4).max(f64::MIN_POSITIVE),
            "Zero" => 10. / 4_294_967_296.,
            _ => document.absolute(),
        };
        Tolerance::try_new(absolute, document.relative(), document.angular())
            .expect("positive finite polygon Boolean tolerance")
    }
}
pub(super) struct Arguments {
    pub options: Options,
    pub first: Option<Vec<ObjectId>>,
    pub second: Option<Vec<ObjectId>>,
}
impl Arguments {
    pub fn parse(kind: Kind, options: Options, args: &[&str]) -> Result<Self, CommandError> {
        let mut result = Self {
            options,
            first: None,
            second: None,
        };
        let (mut tolerance, mut delete, mut index) = (false, false, 0);
        while index < args.len() {
            let (name, value, consumed) = orient_option(args, index, USAGE)?;
            if option_name_eq(name, "Tolerance") && !tolerance {
                result.options.tolerance = ["Auto", "Document", "Zero"]
                    .into_iter()
                    .find(|mode| option_name_eq(value, mode))
                    .ok_or(CommandError::Usage(USAGE))?;
                tolerance = true;
            } else if kind == Kind::Difference && option_name_eq(name, "DeleteInput") && !delete {
                result.options.delete_cutters =
                    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                delete = true;
            } else if kind != Kind::Union
                && option_name_eq(name, "FirstSet")
                && result.first.is_none()
            {
                result.first = Some(boolean_solids::parse_ids(value, USAGE)?);
            } else if kind != Kind::Union
                && option_name_eq(name, "SecondSet")
                && result.second.is_none()
            {
                result.second = Some(boolean_solids::parse_ids(value, USAGE)?);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            index += consumed;
        }
        if result.second.is_some() && result.first.is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(result)
    }
    pub fn sets(
        &self,
        kind: Kind,
        doc: &Document,
        post: bool,
    ) -> Result<(Vec<ObjectId>, Vec<ObjectId>), CommandError> {
        if kind == Kind::Union {
            let ids = if post {
                doc.selected_objects().collect::<Vec<_>>()
            } else {
                doc.objects().filter(|o| doc.is_selected(o.id())).collect()
            };
            let ids = ids
                .into_iter()
                .filter(|o| ObjectSelectionFilter::EdgeAnalysis.accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if ids.len() < 2 {
                return Err(CommandError::MeshBooleanRequiresObjects);
            }
            return Ok((ids, vec![]));
        }
        let (Some(first), Some(second)) = (&self.first, &self.second) else {
            return Err(CommandError::Usage(USAGE));
        };
        if first.is_empty() || second.is_empty() {
            return Err(CommandError::MeshBooleanRequiresObjects);
        }
        Ok((first.clone(), second.clone()))
    }
}
