use std::collections::BTreeMap;
use ir::{self, Anonymize};
use chalk_parse::ast::Identifier;
use errors::*;

crate type TypeIds = BTreeMap<ir::Identifier, ir::ItemId>;
crate type TypeKinds = BTreeMap<ir::ItemId, ir::TypeKind>;
crate type AssociatedTyInfos = BTreeMap<(ir::ItemId, ir::Identifier), AssociatedTyInfo>;
crate type ParameterMap = BTreeMap<ir::ParameterKind<ir::Identifier>, usize>;

#[derive(Clone, Debug)]
crate struct Env<'k> {
    type_ids: &'k TypeIds,
    type_kinds: &'k TypeKinds,
    associated_ty_infos: &'k AssociatedTyInfos,
    parameter_map: ParameterMap,
    traits_in_scope: BTreeMap<ir::ItemId, ir::TraitRef>,
}

#[derive(Debug, PartialEq, Eq)]
crate struct AssociatedTyInfo {
    crate id: ir::ItemId,
    crate addl_parameter_kinds: Vec<ir::ParameterKind<ir::Identifier>>,
}

crate enum NameLookup {
    Type(ir::ItemId),
    Parameter(usize),
}

crate enum LifetimeLookup {
    Parameter(usize),
}

crate const SELF: &str = "Self";

impl<'k> Env<'k> {
    crate fn empty(
        type_ids: &'k TypeIds,
        type_kinds: &'k TypeKinds,
        associated_ty_infos: &'k AssociatedTyInfos
    ) -> Self
    {
        Env {
            type_ids: &type_ids,
            type_kinds: &type_kinds,
            associated_ty_infos: &associated_ty_infos,
            parameter_map: BTreeMap::new(),
            traits_in_scope: BTreeMap::new(),
        }
    }

    crate fn trait_in_scope(&mut self, trait_ref: ir::TraitRef) {
        self.traits_in_scope.insert(trait_ref.trait_id, trait_ref);
    }

    crate fn resolve_unselected_projection_ty(&self, ty: ir::UnselectedProjectionTy)
        -> Result<ir::ProjectionTy>
    {
        let candidates: Vec<_> = self.associated_ty_infos
            .iter()
            .filter(|(key, _)| key.1 == ty.type_name)
            .filter_map(|(key, info)| {
                self.traits_in_scope.get(&key.0).map(|trait_ref| {
                    (trait_ref, info.id)
                })
            })
            .collect();
        
        if candidates.len() != 1 {
            bail!("ambiguous associated ty {}", ty.type_name);
        }

        let (trait_ref, associated_ty_id) = candidates[0];
        let projection_ty = ir::ProjectionTy {
            associated_ty_id,
            parameters: ty.parameters
                .into_iter()
                .chain(trait_ref.parameters.clone())
                .collect(),
        };

        Ok(projection_ty)
    }

    crate fn get_assoc_ty_info(&self, key: &(ir::ItemId, ir::Identifier))
        -> Option<&AssociatedTyInfo>
    {
        self.associated_ty_infos.get(key)
    }

    crate fn lookup(&self, name: Identifier) -> Result<NameLookup> {
        if let Some(k) = self.parameter_map.get(&ir::ParameterKind::Ty(name.str)) {
            return Ok(NameLookup::Parameter(*k));
        }

        if let Some(id) = self.type_ids.get(&name.str) {
            return Ok(NameLookup::Type(*id));
        }

        bail!(ErrorKind::InvalidTypeName(name))
    }

    crate fn lookup_lifetime(&self, name: Identifier) -> Result<LifetimeLookup> {
        if let Some(k) = self.parameter_map
            .get(&ir::ParameterKind::Lifetime(name.str))
        {
            return Ok(LifetimeLookup::Parameter(*k));
        }

        bail!("invalid lifetime name: {:?}", name.str);
    }

    crate fn type_kind(&self, id: ir::ItemId) -> &ir::TypeKind {
        &self.type_kinds[&id]
    }

    /// Introduces new parameters, shifting the indices of existing
    /// parameters to accommodate them. The indices of the new binders
    /// will be assigned in order as they are iterated.
    crate fn introduce<I>(&self, binders: I) -> Result<Self>
    where
        I: IntoIterator<Item = ir::ParameterKind<ir::Identifier>>,
        I::IntoIter: ExactSizeIterator,
    {
        let binders = binders.into_iter().enumerate().map(|(i, k)| (k, i));
        let len = binders.len();
        let parameter_map: ParameterMap = self.parameter_map
            .iter()
            .map(|(&k, &v)| (k, v + len))
            .chain(binders)
            .collect();
        if parameter_map.len() != self.parameter_map.len() + len {
            bail!("duplicate parameters");
        }
        Ok(Env {
            parameter_map,
            ..*self
        })
    }

    crate fn in_binders<I, T, OP>(&self, binders: I, op: OP) -> Result<ir::Binders<T>>
    where
        I: IntoIterator<Item = ir::ParameterKind<ir::Identifier>>,
        I::IntoIter: ExactSizeIterator,
        OP: FnOnce(&mut Self) -> Result<T>,
    {
        let binders: Vec<_> = binders.into_iter().collect();
        let env = self.introduce(binders.iter().cloned())?;
        Ok(ir::Binders {
            binders: binders.anonymize(),
            value: op(&env)?,
        })
    }
}