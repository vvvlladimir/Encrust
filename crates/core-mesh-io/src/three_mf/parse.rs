//! A model part's XML read into a [`Model`] in one streaming pass. Elements of a namespace
//! not read here are skipped, as the core specification asks of a consumer (§2.3.1).

use std::io::BufRead;
use std::str::FromStr;

use core_geometry::{Mat4, Vec2, Vec3};
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};
use quick_xml::{NsReader, XmlVersion};

use super::model::{MeshPart, Model, Object, Placed, Shape, TextureGroup, Triangle};

const CORE: &[u8] = b"http://schemas.microsoft.com/3dmanufacturing/core/2015/02";
const MATERIAL: &[u8] = b"http://schemas.microsoft.com/3dmanufacturing/material/2015/02";
const PRODUCTION: &[u8] = b"http://schemas.microsoft.com/3dmanufacturing/production/2015/06";

#[derive(Clone, Copy)]
enum Space {
    Core,
    Material,
    Other,
}

impl Space {
    /// A part declaring no namespace at all is read as core, as the files that do so mean it.
    fn of(resolved: &ResolveResult<'_>) -> Self {
        match resolved {
            ResolveResult::Bound(Namespace(CORE)) | ResolveResult::Unbound => Self::Core,
            ResolveResult::Bound(Namespace(MATERIAL)) => Self::Material,
            _ => Self::Other,
        }
    }
}

/// Reads one model part. Only the root part may place an object from another part, so `root`
/// says whether a component's `p:path` is allowed.
pub(super) fn parse(source: impl BufRead, root: bool) -> Result<Model, String> {
    let mut reader = NsReader::from_reader(source);
    let mut parser = Parser {
        model: Model::default(),
        object: None,
        group: None,
        root,
    };
    let (mut buffer, mut skipped) = (Vec::new(), Vec::new());
    loop {
        let (resolved, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|error| error.to_string())?;
        let space = Space::of(&resolved);
        match event {
            Event::Start(element) if matches!(space, Space::Other) => {
                parser.open(space, &element, reader.resolver())?;
                reader
                    .read_to_end_into(element.name(), &mut skipped)
                    .map_err(|error| error.to_string())?;
                skipped.clear();
            }
            Event::Start(element) => parser.open(space, &element, reader.resolver())?,
            Event::Empty(element) => {
                parser.open(space, &element, reader.resolver())?;
                parser.close(space, element.local_name().as_ref());
            }
            Event::End(element) => parser.close(space, element.local_name().as_ref()),
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(parser.model)
}

/// The model so far, and the object or texture group whose children are being read.
struct Parser {
    model: Model,
    object: Option<(u32, Object)>,
    group: Option<(u32, TextureGroup)>,
    root: bool,
}

impl Parser {
    fn open(
        &mut self,
        space: Space,
        element: &BytesStart<'_>,
        names: &NamespaceResolver,
    ) -> Result<(), String> {
        match (space, element.local_name().as_ref()) {
            (Space::Core, b"model") => self.model.mm_per_unit = mm_per_unit(element)?,
            (Space::Core, b"object") => self.object = Some(object(element)?),
            (Space::Core, b"mesh") => self.shape(Shape::Mesh(MeshPart::default())),
            (Space::Core, b"components") => self.shape(Shape::Components(Vec::new())),
            (Space::Core, b"vertex") => {
                if let Some(mesh) = self.mesh() {
                    mesh.vertices.push(vertex(element)?);
                }
            }
            (Space::Core, b"triangle") => {
                if let Some(mesh) = self.mesh() {
                    mesh.triangles.push(triangle(element)?);
                }
            }
            (Space::Core, b"component") => self.component(placed(element, names, "component")?)?,
            (Space::Core, b"item") => self.model.build.push(placed(element, names, "item")?),
            (Space::Material, b"texture2d") => {
                let (id, path) = texture(element)?;
                self.model.textures.entry(id).or_insert(path);
            }
            (Space::Material, b"texture2dgroup") => self.group = Some(texture_group(element)?),
            (Space::Material, b"tex2coord") => {
                if let Some((_, group)) = &mut self.group {
                    group.coordinates.push(coordinate(element)?);
                }
            }
            (Space::Other, b"booleanshape") => self.shape(Shape::Unsupported("3MF boolean shapes")),
            (Space::Other, b"displacementmesh") => {
                self.shape(Shape::Unsupported("3MF displacement meshes"));
            }
            _ => {}
        }
        Ok(())
    }

    /// Files the closed resource; of two sharing an id, the first is kept.
    fn close(&mut self, space: Space, local: &[u8]) {
        match (space, local) {
            (Space::Core, b"object") => {
                if let Some((id, object)) = self.object.take() {
                    self.model.objects.entry(id).or_insert(object);
                }
            }
            (Space::Material, b"texture2dgroup") => {
                if let Some((id, group)) = self.group.take() {
                    self.model.groups.entry(id).or_insert(group);
                }
            }
            _ => {}
        }
    }

    /// Gives the open object its shape, unless it already has one.
    fn shape(&mut self, shape: Shape) {
        if let Some((_, object)) = &mut self.object
            && matches!(object.shape, Shape::Empty)
        {
            object.shape = shape;
        }
    }

    fn mesh(&mut self) -> Option<&mut MeshPart> {
        match &mut self.object.as_mut()?.1.shape {
            Shape::Mesh(mesh) => Some(mesh),
            _ => None,
        }
    }

    fn component(&mut self, component: Placed) -> Result<(), String> {
        if !self.root && component.path.is_some() {
            return Err("a component outside the root model names another part".to_owned());
        }
        if let Some((_, object)) = &mut self.object
            && let Shape::Components(components) = &mut object.shape
        {
            components.push(component);
        }
        Ok(())
    }
}

fn mm_per_unit(element: &BytesStart<'_>) -> Result<f32, String> {
    let Some(unit) = element
        .try_get_attribute("unit")
        .map_err(|error| error.to_string())?
    else {
        return Ok(1.0);
    };
    match unit.value.as_ref() {
        b"micron" => Ok(0.001),
        b"millimeter" => Ok(1.0),
        b"centimeter" => Ok(10.0),
        b"inch" => Ok(25.4),
        b"foot" => Ok(304.8),
        b"meter" => Ok(1000.0),
        other => Err(format!("the unit {}", String::from_utf8_lossy(other))),
    }
}

fn object(element: &BytesStart<'_>) -> Result<(u32, Object), String> {
    let (mut id, mut object) = (None, Object::default());
    for attribute in attributes(element) {
        let attribute = attribute?;
        match attribute.key.as_ref() {
            b"id" => id = Some(number(&attribute, "object")?),
            b"pid" => object.pid = Some(number(&attribute, "object")?),
            b"pindex" => object.pindex = Some(number(&attribute, "object")?),
            _ => {}
        }
    }
    Ok((required(id, "object", "id")?, object))
}

fn vertex(element: &BytesStart<'_>) -> Result<Vec3, String> {
    let mut at = [None; 3];
    for attribute in attributes(element) {
        let attribute = attribute?;
        let axis = match attribute.key.as_ref() {
            b"x" => 0,
            b"y" => 1,
            b"z" => 2,
            _ => continue,
        };
        at[axis] = Some(finite(&attribute, "vertex")?);
    }
    Ok(Vec3::new(
        required(at[0], "vertex", "x")?,
        required(at[1], "vertex", "y")?,
        required(at[2], "vertex", "z")?,
    ))
}

fn triangle(element: &BytesStart<'_>) -> Result<Triangle, String> {
    let (mut vertices, mut properties, mut pid) = ([None; 3], [None; 3], None);
    for attribute in attributes(element) {
        let attribute = attribute?;
        let slot = match attribute.key.as_ref() {
            b"v1" => &mut vertices[0],
            b"v2" => &mut vertices[1],
            b"v3" => &mut vertices[2],
            b"p1" => &mut properties[0],
            b"p2" => &mut properties[1],
            b"p3" => &mut properties[2],
            b"pid" => &mut pid,
            _ => continue,
        };
        *slot = Some(number(&attribute, "triangle")?);
    }
    Ok(Triangle {
        vertices: [
            required(vertices[0], "triangle", "v1")?,
            required(vertices[1], "triangle", "v2")?,
            required(vertices[2], "triangle", "v3")?,
        ],
        pid,
        properties,
    })
}

/// A build item or component. Its `p:path` is the production extension's attribute.
fn placed(element: &BytesStart<'_>, names: &NamespaceResolver, of: &str) -> Result<Placed, String> {
    let (mut objectid, mut transform, mut path) = (None, Mat4::IDENTITY, None);
    for attribute in attributes(element) {
        let attribute = attribute?;
        match names.resolve_attribute(attribute.key) {
            (ResolveResult::Unbound, local) if local.as_ref() == b"objectid" => {
                objectid = Some(number(&attribute, of)?);
            }
            (ResolveResult::Unbound, local) if local.as_ref() == b"transform" => {
                transform = matrix(&attribute.value)?;
            }
            (ResolveResult::Bound(Namespace(PRODUCTION)), local) if local.as_ref() == b"path" => {
                path = Some(text(&attribute)?);
            }
            _ => {}
        }
    }
    Ok(Placed {
        objectid: required(objectid, of, "objectid")?,
        transform,
        path,
    })
}

fn texture(element: &BytesStart<'_>) -> Result<(u32, String), String> {
    let (mut id, mut path) = (None, None);
    for attribute in attributes(element) {
        let attribute = attribute?;
        match attribute.key.as_ref() {
            b"id" => id = Some(number(&attribute, "texture2d")?),
            b"path" => path = Some(text(&attribute)?),
            _ => {}
        }
    }
    Ok((
        required(id, "texture2d", "id")?,
        required(path, "texture2d", "path")?,
    ))
}

fn texture_group(element: &BytesStart<'_>) -> Result<(u32, TextureGroup), String> {
    let (mut id, mut texture) = (None, None);
    for attribute in attributes(element) {
        let attribute = attribute?;
        match attribute.key.as_ref() {
            b"id" => id = Some(number(&attribute, "texture2dgroup")?),
            b"texid" => texture = Some(number(&attribute, "texture2dgroup")?),
            _ => {}
        }
    }
    let group = TextureGroup {
        texture: required(texture, "texture2dgroup", "texid")?,
        coordinates: Vec::new(),
    };
    Ok((required(id, "texture2dgroup", "id")?, group))
}

fn coordinate(element: &BytesStart<'_>) -> Result<Vec2, String> {
    let (mut u, mut v) = (None, None);
    for attribute in attributes(element) {
        let attribute = attribute?;
        match attribute.key.as_ref() {
            b"u" => u = Some(finite(&attribute, "tex2coord")?),
            b"v" => v = Some(finite(&attribute, "tex2coord")?),
            _ => {}
        }
    }
    Ok(Vec2::new(
        required(u, "tex2coord", "u")?,
        required(v, "tex2coord", "v")?,
    ))
}

/// Twelve numbers placing a row vector: `m00 m01 m02 m10 … m32`, so its rows are glam's columns.
fn matrix(value: &[u8]) -> Result<Mat4, String> {
    let refused = || format!("the transform {}", String::from_utf8_lossy(value));
    let words = std::str::from_utf8(value).map_err(|_| refused())?;
    let numbers: Vec<f32> = words
        .split_ascii_whitespace()
        .map(|word| word.parse().ok().filter(|number: &f32| number.is_finite()))
        .collect::<Option<_>>()
        .ok_or_else(refused)?;
    let [m00, m01, m02, m10, m11, m12, m20, m21, m22, m30, m31, m32] =
        <[f32; 12]>::try_from(numbers).map_err(|_| refused())?;
    Ok(Mat4::from_cols_array(&[
        m00, m01, m02, 0.0, //
        m10, m11, m12, 0.0, //
        m20, m21, m22, 0.0, //
        m30, m31, m32, 1.0,
    ]))
}

/// Attributes unchecked for repeats: a vertex or triangle is read millions of times, and a
/// repeated one only means the last of them wins.
fn attributes<'a>(
    element: &'a BytesStart<'a>,
) -> impl Iterator<Item = Result<Attribute<'a>, String>> {
    let mut all = element.attributes();
    all.with_checks(false);
    all.map(|attribute| attribute.map_err(|error| error.to_string()))
}

fn number<T: FromStr>(attribute: &Attribute<'_>, of: &str) -> Result<T, String> {
    std::str::from_utf8(&attribute.value)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| {
            format!(
                "the {of} {} {} is not a number",
                String::from_utf8_lossy(attribute.key.as_ref()),
                String::from_utf8_lossy(&attribute.value)
            )
        })
}

fn finite(attribute: &Attribute<'_>, of: &str) -> Result<f32, String> {
    let value: f32 = number(attribute, of)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "the {of} {} is {value}",
            String::from_utf8_lossy(attribute.key.as_ref())
        ))
    }
}

fn text(attribute: &Attribute<'_>) -> Result<String, String> {
    attribute
        .normalized_value(XmlVersion::Implicit1_0)
        .map(std::borrow::Cow::into_owned)
        .map_err(|error| error.to_string())
}

fn required<T>(value: Option<T>, of: &str, name: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("a {of} without {name}"))
}
