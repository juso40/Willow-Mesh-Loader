//! Reviewed definition layouts and native range rebuilding while users remain detached.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
};
use unrealsdk_rs::{
    objects::{
        self, fname_to_string, is_kind_of, objects as all_objects, property_array_dim,
        property_class_name, property_element_size, property_inner, property_offset,
        property_struct, struct_fields,
    },
    types::{FName, UObject, ZProperty},
};

use super::PartRange;
use crate::{
    Error, Result,
    encoding::Bounds,
    engine::{
        abi::ARRAY_HEADER_BYTES,
        construct::{self, Region, load, read, read_array},
        memory::word,
        native::{GestaltInitialize, NativeFunction},
        reflection,
    },
};

const CLASS: &str = "GearboxFramework.GestaltSkeletalMeshDefinition";
const MESH: usize = 0x3c;
const FLAGS: usize = 0x40;
const INFOS: usize = 0x44;
const ACCESSORIES: usize = 0x50;
const SOCKETS: usize = 0x5c;
const BOUNDS: usize = 0x68;
const SIZE: usize = 0x74;
const PART_SIZE: usize = 0x18;
const BOUNDS_SIZE: usize = 0x24;
const COMPONENT_DATA: usize = 0x228;
pub(crate) const COMPONENT_MASTER: usize = COMPONENT_DATA + 0x0c;
const COMPONENT_LODS: usize = COMPONENT_DATA + 0x10;
const COMPONENT_SOCKETS: usize = COMPONENT_DATA + 0x1c;
const COMPONENT_ACCESSORIES: usize = COMPONENT_DATA + 0x28;

static INITIALIZE: NativeFunction<GestaltInitialize> = NativeFunction::new(
    "InitGestaltMeshDataFromParts",
    "55 8B EC 81 EC C0 00 00 00 8B 45 0C 53 8B 5D 08 56 57 8B F9 50 8D 8B 28 02 00 00 89 7D FC E8 ?? ?? ?? ?? 8B 4F 3C 89 8B 34 02 00 00 6A 00 8D 8B 38 02 00 00 E8 ?? ?? ?? ?? 33 D2 89 93 48 02 00 00 39 93 4C 02 00 00 74 3D 8B 83 44 02 00 00 89 93 4C 02 00 00 89 45 08 3B C2 74 2A 8B 0D ?? ?? ?? ??",
    &[],
);

fn field(
    st: *const c_void,
    name: &str,
    kind: &str,
    offset: usize,
    size: usize,
) -> Result<*mut ZProperty> {
    let prop = struct_fields(st)
        .into_iter()
        .find(|p| objects::object_name_text(p.cast()) == name)
        .ok_or_else(|| Error(format!("Gestalt property {name} unavailable")))?;
    if property_class_name(prop).as_deref() != Some(kind)
        || property_offset(prop) != Some(offset as i32)
        || property_element_size(prop) != Some(size as i32)
        || property_array_dim(prop) != Some(1)
    {
        return Err(Error(format!("Gestalt property {name} layout differs")));
    }
    Ok(prop)
}

fn array_struct(
    st: *const c_void,
    name: &str,
    offset: usize,
    size: usize,
) -> Result<*const c_void> {
    let prop = field(st, name, "ArrayProperty", offset, ARRAY_HEADER_BYTES)?;
    let inner = property_inner(prop)
        .ok_or_else(|| Error(format!("Gestalt array {name} inner unavailable")))?;
    if property_class_name(inner).as_deref() != Some("StructProperty")
        || property_element_size(inner) != Some(size as i32)
    {
        return Err(Error(format!(
            "Gestalt array {name} element layout differs"
        )));
    }
    property_struct(inner)
        .map(|st| st.cast_const().cast())
        .ok_or_else(|| Error(format!("Gestalt array {name} structure unavailable")))
}

fn schema(class: *const c_void) -> Result<()> {
    field(class, "GestaltSkeletalMesh", "ObjectProperty", MESH, 4)?;
    field(class, "bStaticMeshRigidBodyMesh", "BoolProperty", FLAGS, 4)?;
    let info = array_struct(class, "GestaltInfos", INFOS, ARRAY_HEADER_BYTES)?;
    let part = array_struct(info, "Parts", 0, PART_SIZE)?;
    for (name, kind, offset, size) in [
        ("SkeletalMeshFragment", "ObjectProperty", 0, 4),
        ("SkeletalMeshFragmentName", "NameProperty", 4, 8),
        ("MaterialIndex", "IntProperty", 12, 4),
        ("FirstIndex", "IntProperty", 16, 4),
        ("NumPrimitives", "IntProperty", 20, 4),
    ] {
        field(part, name, kind, offset, size)?;
    }
    array_struct(class, "GestaltAccessoryNames", ACCESSORIES, 8)?;
    array_struct(class, "GestaltSocketMappings", SOCKETS, 0x1c)?;
    let bounds = array_struct(class, "GestaltPartBounds", BOUNDS, BOUNDS_SIZE)?;
    field(bounds, "SkeletalMeshFragmentName", "NameProperty", 0, 8)?;
    field(bounds, "ReferencePoseBounds", "StructProperty", 8, 0x1c)?;
    let component = construct::require_class("Engine.SkeletalMeshComponent")?;
    let data = field(
        component.cast(),
        "GestaltData",
        "StructProperty",
        COMPONENT_DATA,
        0x54,
    )?;
    let data = property_struct(data)
        .ok_or_else(|| Error("Gestalt component-data structure unavailable".into()))?;
    field(
        data.cast(),
        "PartMeshNames",
        "ArrayProperty",
        0,
        ARRAY_HEADER_BYTES,
    )?;
    field(data.cast(), "SkeletalMesh", "ObjectProperty", 12, 4)?;
    let lod = array_struct(data.cast(), "LODModelData", 16, ARRAY_HEADER_BYTES)?;
    let fragment = array_struct(lod, "GestaltFragments", 0, 12)?;
    for (name, offset) in [
        ("MaterialIndex", 0),
        ("FirstIndex", 4),
        ("NumPrimitives", 8),
    ] {
        field(fragment, name, "IntProperty", offset, 4)?;
    }
    array_struct(data.cast(), "SocketMappings", 0x1c, 16)?;
    array_struct(data.cast(), "GestaltAccessoryList", 0x28, 12)?;
    field(
        data.cast(),
        "GestaltPartMatricesContainer",
        "ObjectProperty",
        0x34,
        4,
    )?;
    field(
        data.cast(),
        "ReferencePoseBounds",
        "StructProperty",
        0x38,
        0x1c,
    )?;
    Ok(())
}

fn name(bytes: &[u8], offset: usize) -> Result<FName> {
    Ok(FName {
        index: word(bytes, offset)?,
        number: word(bytes, offset + 4)?,
    })
}

fn name_bytes(name: &FName) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&name.index.to_le_bytes());
    bytes[4..].copy_from_slice(&name.number.to_le_bytes());
    bytes
}

pub(crate) struct Definition {
    object: *mut UObject,
    master: usize,
    pub(crate) mesh_path: String,
    pub(crate) parts: Vec<PartRange>,
    names: BTreeMap<String, FName>,
    bounds: Vec<u8>,
    snapshots: Vec<(usize, Vec<u8>)>,
}

impl Definition {
    pub(crate) fn read(path: &str) -> Result<Self> {
        let class = construct::require_class(CLASS)?;
        schema(class.cast())?;
        let object = reflection::loaded_object(path, CLASS)?;
        construct::verify_live(
            "Gestalt definition",
            construct::flags_of("Gestalt definition", object)?,
        )?;
        let address = object as usize;
        if load(address + FLAGS)? & 1 != 0 {
            return Err(Error(
                "Gestalt updates refuse rigid-body definitions".into(),
            ));
        }
        let master = load(address + MESH)? as usize;
        let mesh_path = construct::object_path("Gestalt master", master as *mut UObject)?;
        for other in all_objects().ok_or_else(|| Error("GObjects unavailable".into()))? {
            if other.is_null() || other == object {
                continue;
            }
            if is_kind_of(other, class)
                .ok_or_else(|| Error("Gestalt definition classification unavailable".into()))?
                && load(other as usize + MESH)? as usize == master
            {
                return Err(Error(
                    "Gestalt update requires an exclusive master mesh".into(),
                ));
            }
        }
        let infos = read_array(address + INFOS, ARRAY_HEADER_BYTES, 1)?;
        if infos.len() != ARRAY_HEADER_BYTES {
            return Err(Error("Gestalt updates require one LOD0 part table".into()));
        }
        let part_header = load(address + INFOS)? as usize;
        let raw_parts = read_array(part_header, PART_SIZE, i32::MAX as usize)?;
        let bounds = read_array(address + BOUNDS, BOUNDS_SIZE, i32::MAX as usize)?;
        read_array(address + ACCESSORIES, 8, i32::MAX as usize)?;
        read_array(address + SOCKETS, 0x1c, i32::MAX as usize)?;
        let mut parts = Vec::new();
        let mut names = BTreeMap::new();
        for row in raw_parts.chunks_exact(PART_SIZE) {
            let native_name = name(row, 4)?;
            let mut text = fname_to_string(&native_name)
                .ok_or_else(|| Error("Gestalt fragment name unavailable".into()))?
                .text;
            if native_name.number != 0 {
                text.push_str(&format!("_{}", native_name.number - 1));
            }
            if text.is_empty() {
                return Err(Error("Gestalt updates require named fragments".into()));
            }
            if let Some(previous) = names.insert(text.clone(), native_name)
                && name_bytes(&previous) != name_bytes(&native_name)
            {
                return Err(Error(format!(
                    "ambiguous displayed Gestalt identity {text}"
                )));
            }
            parts.push(PartRange {
                name: text,
                material_slot: word(row, 12)? as usize,
                first_index: word(row, 16)? as usize,
                triangles: word(row, 20)? as usize,
            });
        }
        let mut snapshots = vec![
            (address + MESH, read(address + MESH, SIZE - MESH)?),
            (part_header, infos),
        ];
        if !raw_parts.is_empty() {
            snapshots.push((load(part_header)? as usize, raw_parts));
        }
        if !bounds.is_empty() {
            snapshots.push((load(address + BOUNDS)? as usize, bounds.clone()));
        }
        Ok(Self {
            object,
            master,
            mesh_path,
            parts,
            names,
            bounds,
            snapshots,
        })
    }
}

struct Selected {
    component: usize,
    names: Vec<u8>,
    accessories: Vec<u8>,
    fragments: Vec<u8>,
}

pub(crate) struct Publication {
    definition: Definition,
    infos: Region,
    bounds: Region,
    bound_count: u32,
    rows: Vec<u8>,
    initialize: GestaltInitialize,
    selected: Vec<Selected>,
}

impl Publication {
    pub(crate) fn stage(
        base: usize,
        definition: Definition,
        parts: &[PartRange],
        updated: &BTreeMap<String, Bounds>,
    ) -> Result<Self> {
        let initialize = INITIALIZE.resolve(base)?;
        let mut rows = Vec::new();
        for part in parts {
            let native_name = definition
                .names
                .get(&part.name)
                .ok_or_else(|| Error("Gestalt update introduced an unknown identity".into()))?;
            rows.extend_from_slice(&0u32.to_le_bytes());
            rows.extend_from_slice(&name_bytes(native_name));
            for value in [part.material_slot, part.first_index, part.triangles] {
                rows.extend_from_slice(
                    &i32::try_from(value)
                        .map_err(|_| Error("Gestalt range exceeds native storage".into()))?
                        .to_le_bytes(),
                );
            }
        }
        let mut bounds = definition.bounds.clone();
        for (text, bound) in updated {
            let key = name_bytes(&definition.names[text]);
            let encoded: Vec<_> = bound
                .origin
                .iter()
                .chain(&bound.box_extent)
                .chain([&bound.sphere_radius])
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let mut found = false;
            for row in bounds.chunks_exact_mut(BOUNDS_SIZE) {
                if row[..8] == key {
                    row[8..].copy_from_slice(&encoded);
                    found = true;
                }
            }
            if !found {
                bounds.extend_from_slice(&key);
                bounds.extend_from_slice(&encoded);
            }
        }
        let part_count = i32::try_from(parts.len())
            .map_err(|_| Error("Gestalt part count exceeds native storage".into()))?;
        let bound_count = i32::try_from(bounds.len() / BOUNDS_SIZE)
            .map_err(|_| Error("Gestalt bounds count exceeds native storage".into()))?;
        let native_parts = construct::retain_bytes(&rows)?;
        let infos = construct::retain_allocation(ARRAY_HEADER_BYTES)?;
        infos.header(0, native_parts.address, part_count as u32)?;
        let bounds = construct::retain_bytes(&bounds)?;
        Ok(Self {
            definition,
            infos,
            bounds,
            bound_count: bound_count as u32,
            rows,
            initialize,
            selected: Vec::new(),
        })
    }

    pub(crate) fn mesh_path(&self) -> &str {
        &self.definition.mesh_path
    }

    pub(crate) fn references(&self, component: usize) -> Result<bool> {
        Ok(load(component + COMPONENT_MASTER)? as usize == self.definition.master)
    }

    pub(crate) fn capture(&mut self, users: &[usize]) -> Result<()> {
        for &component in users {
            if !self.references(component)? {
                continue;
            }
            let names = read_array(component + COMPONENT_DATA, 8, i32::MAX as usize)?;
            let lods = read_array(component + COMPONENT_LODS, ARRAY_HEADER_BYTES, 1)?;
            if !lods.is_empty() {
                read_array(
                    load(component + COMPONENT_LODS)? as usize,
                    12,
                    i32::MAX as usize,
                )?;
            }
            read_array(component + COMPONENT_SOCKETS, 16, i32::MAX as usize)?;
            read_array(component + COMPONENT_ACCESSORIES, 12, i32::MAX as usize)?;
            let keys: BTreeSet<_> = names.chunks_exact(8).collect();
            let fragments = self
                .rows
                .chunks_exact(PART_SIZE)
                .filter(|row| keys.contains(&row[4..12]))
                .flat_map(|row| row[12..24].iter().copied())
                .collect();
            // Accessory bone indices and matrix ownership depend on the unchanged skeleton,
            // not on fragment offsets. The common initializer leaves these fields intact.
            let accessories = read(component + COMPONENT_ACCESSORIES, ARRAY_HEADER_BYTES + 4)?;
            self.selected.push(Selected {
                component,
                names,
                accessories,
                fragments,
            });
        }
        construct::retain_object("updated Gestalt definition", self.definition.object)
    }

    pub(crate) fn verify(&self) -> Result<()> {
        for (address, bytes) in &self.definition.snapshots {
            if read(*address, bytes.len())? != *bytes {
                return Err(Error(
                    "Gestalt definition changed during update preparation".into(),
                ));
            }
        }
        for selected in &self.selected {
            if !self.references(selected.component)?
                || read_array(selected.component + COMPONENT_DATA, 8, i32::MAX as usize)?
                    != selected.names
                || read(
                    selected.component + COMPONENT_ACCESSORIES,
                    ARRAY_HEADER_BYTES + 4,
                )? != selected.accessories
            {
                return Err(Error(
                    "Gestalt component selection changed during detachment".into(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn install(&self) -> Result<()> {
        let destination = Region {
            address: self.definition.object as usize,
            length: SIZE,
        };
        destination.header(INFOS, self.infos.address, 1)?;
        destination.header(BOUNDS, self.bounds.address, self.bound_count)
    }

    pub(crate) fn refresh(&self) -> Result<()> {
        for selected in &self.selected {
            let count = (selected.names.len() / 8) as u32;
            let view = [selected.names.as_ptr() as usize as u32, count, count];
            // SAFETY: reviewed thiscall entry, checked definition/component layouts,
            // completed render fence and an independent live FName-array copy.
            unsafe {
                (self.initialize)(
                    self.definition.object,
                    selected.component as *mut UObject,
                    &view,
                )
            };
            let lods = read_array(selected.component + COMPONENT_LODS, ARRAY_HEADER_BYTES, 1)?;
            if lods.len() != ARRAY_HEADER_BYTES
                || read_array(
                    load(selected.component + COMPONENT_LODS)? as usize,
                    12,
                    i32::MAX as usize,
                )? != selected.fragments
                || read_array(selected.component + COMPONENT_DATA, 8, i32::MAX as usize)?
                    != selected.names
                || read(
                    selected.component + COMPONENT_ACCESSORIES,
                    ARRAY_HEADER_BYTES + 4,
                )? != selected.accessories
                || load(selected.component + COMPONENT_MASTER)? as usize != self.definition.master
            {
                return Err(Error("native Gestalt component refresh did not preserve its selection and accessory state".into()));
            }
        }
        Ok(())
    }
}
