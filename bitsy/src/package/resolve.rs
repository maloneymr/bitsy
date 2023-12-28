use std::collections::BTreeSet;
use super::*;

pub fn resolve(package: &ast::Package) -> Vec<Item> {
    let mut items: BTreeMap<String, Item> = BTreeMap::new();
    for item in order_items(package) {
        let item = resolve_item(item, &items);
        items.insert(item.name().to_string(), item);
    }

    items.into_iter().map(|(_name, item)| item).collect()
}

fn order_items(package: &ast::Package) -> Vec<&ast::Item> {
    use petgraph::graph::{DiGraph, NodeIndex};
    use petgraph::algo::toposort;

    let mut items: BTreeMap<String, (NodeIndex, &ast::Item)> = BTreeMap::new();
    let mut name_by_node: BTreeMap<NodeIndex, String> = BTreeMap::new();
    let mut graph = DiGraph::new();

    for item in &package.items {
        let node = graph.add_node(item.name().to_string());
        items.insert(item.name().to_string(), (node, item));
        name_by_node.insert(node, item.name().to_string());
    }

    for item in &package.items {
        let (node, _item) = items[item.name()];
        items.insert(item.name().to_string(), (node, item));

        for item_dependency in item_dependencies(item) {
            if let Some((dependency, _item)) = items.get(&item_dependency) {
                graph.add_edge(node, *dependency, ());
            } else {
                panic!("{item_dependency} not found")
            }
        }
    }

    let mut sorted: Vec<NodeIndex> = toposort(&graph, None).unwrap();
    sorted.reverse();

    let mut results = vec![];
    for node in sorted {
        let name = &name_by_node[&node];
        let (_node, item) = items[name];
        results.push(item);
    }
    results
}

fn resolve_item(item: &ast::Item, items: &BTreeMap<String, Item>) -> Item {
    let user_types: Vec<(String, Type)> = items
        .clone()
        .into_iter()
        .filter(|(_name, item)| item.is_typedef())
        .map(|(name, item)| (name, item.as_type().unwrap()))
        .collect();
    let moddefs: Vec<(String, Arc<Component>)> = items
        .clone()
        .into_iter()
        .filter(|(_name, item)| item.is_moddef())
        .map(|(name, item)| (name, item.as_component().unwrap()))
        .collect();
    let fndefs: Vec<(String, Arc<FnDef>)> = items
        .clone()
        .into_iter()
        .filter(|(_name, item)| item.is_fndef())
        .map(|(name, item)| (name, item.as_fndef().unwrap()))
        .collect();

    match item {
        ast::Item::ModDef(moddef) => {
            let ctx = Context::from(user_types.clone().into_iter().collect());
            let mod_ctx = Context::from(moddefs.clone().into_iter().collect());
            let fndef_ctx = Context::from(fndefs.clone().into_iter().collect());
            let moddef = resolve_moddef(moddef, ctx, mod_ctx, fndef_ctx);
            Item::ModDef(moddef.clone())
        },
        ast::Item::ExtDef(moddef) => {
            let ctx = Context::from(user_types.clone().into_iter().collect());
            let mod_ctx = Context::from(moddefs.clone().into_iter().collect());
            let fndef_ctx = Context::from(fndefs.clone().into_iter().collect());
            let moddef = resolve_extmoddef(moddef, ctx, mod_ctx, fndef_ctx);
            Item::ExtDef(moddef.clone())
        },
        ast::Item::EnumTypeDef(typedef) => {
            let typedef = resolve_enum_typedef(typedef);
            Item::EnumTypeDef(typedef)
        },
        ast::Item::StructTypeDef(typedef) => {
            let ctx = Context::from(user_types.clone().into_iter().collect());
            let typedef = resolve_struct_typedef(typedef, ctx);
            Item::StructTypeDef(typedef)
        },
        ast::Item::AltTypeDef(typedef) => {
            let ctx = Context::from(user_types.clone().into_iter().collect());
            let typedef = resolve_alt_typedef(typedef, ctx);
            Item::AltTypeDef(typedef)
        },
        ast::Item::FnDef(fndef) => {
            let ctx = Context::from(user_types.clone().into_iter().collect());
            let fn_ctx = Context::from(fndefs.clone().into_iter().collect());
            let fndef = resolve_fndef(fndef, ctx, fn_ctx);
            Item::FnDef(fndef)
        },
    }
}

fn item_dependencies(item: &ast::Item) -> BTreeSet<String> {
    match item {
        ast::Item::ModDef(moddef) => moddef_dependencies(moddef),
        ast::Item::ExtDef(moddef) => moddef_dependencies(moddef),
        ast::Item::EnumTypeDef(_typedef) => BTreeSet::new(),
        ast::Item::StructTypeDef(typedef) => structtypedef_dependencies(typedef),
        ast::Item::AltTypeDef(typedef) => altypedef_dependencies(typedef),
        ast::Item::FnDef(typedef) => fndef_dependencies(typedef),
    }
}

fn moddef_dependencies(moddef: &ast::ModDef) -> BTreeSet<String> {
    let mut results = vec![];
    let component_names = moddef_component_names(moddef);
    let ast::ModDef(_loc, _name, decls) = moddef;
    for decl in decls {
        results.extend(decl_dependencies(decl, &component_names).into_iter());
    }
    results.into_iter().collect()
}

fn moddef_component_names(moddef: &ast::ModDef) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let ast::ModDef(_loc, _name, decls) = moddef;
    for decl in decls {
        match decl {
            ast::Decl::Mod(_loc, name, _decls) => { result.insert(name.to_string()); },
            ast::Decl::ModInst(_loc, name, _moddef_name) => { result.insert(name.to_string()); },
            ast::Decl::Incoming(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Outgoing(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Node(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Reg(_loc, name, _typ, _reset) => { result.insert(name.to_string()); },
            ast::Decl::Wire(_loc, _wire) => (),
            ast::Decl::When(_loc, _when) => (),
        }
    }
    result
}

fn moddef_component_names_anonymous(decls: &[ast::Decl]) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for decl in decls {
        match decl {
            ast::Decl::Mod(_loc, name, _decls) => { result.insert(name.to_string()); },
            ast::Decl::ModInst(_loc, name, _moddef_name) => { result.insert(name.to_string()); },
            ast::Decl::Incoming(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Outgoing(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Node(_loc, name, _typ) => { result.insert(name.to_string()); },
            ast::Decl::Reg(_loc, name, _typ, _reset) => { result.insert(name.to_string()); },
            ast::Decl::Wire(_loc, _wire) => (),
            ast::Decl::When(_loc, _when) => (),
        }
    }
    result
}

fn decl_dependencies(decl: &ast::Decl, component_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut results = vec![];
    match decl {
        ast::Decl::Mod(_loc, _name, decls) => {
            let component_names = moddef_component_names_anonymous(&*decls);
            for decl in decls {
                results.extend(decl_dependencies(decl, &component_names).into_iter());
            }
        },
        ast::Decl::ModInst(_loc, _name, moddef_name) => results.push(moddef_name.to_string()),
        ast::Decl::Incoming(_loc, _name, typ) => results.extend(type_dependencies(typ)),
        ast::Decl::Outgoing(_loc, _name, typ) => results.extend(type_dependencies(typ)),
        ast::Decl::Node(_loc, _name, typ) => results.extend(type_dependencies(typ)),
        ast::Decl::Reg(_loc, _name, typ, reset) => {
            results.extend(type_dependencies(typ).into_iter());
            if let Some(expr) = reset {
                results.extend(expr_dependencies(expr, component_names).into_iter());
            }
        },
        ast::Decl::Wire(_loc, wire) => {
            let ast::Wire(_loc2, _target, expr, _wire_type) = wire;
            results.extend(expr_dependencies(expr, component_names).into_iter())
        },
        ast::Decl::When(_loc, ast::When(cond, wires)) => {
            results.extend(expr_dependencies(cond, component_names).into_iter());
            for ast::Wire(_loc2, _target, expr, _wire_type) in wires {
                results.extend(expr_dependencies(expr, component_names).into_iter())
            }
        },
    }
    results.into_iter().collect()
}

fn structtypedef_dependencies(typedef: &ast::StructTypeDef) -> BTreeSet<String> {
    typedef
        .fields
        .iter()
        .map(|(_name, typ)| type_dependencies(typ))
        .flatten()
        .collect()
}

fn altypedef_dependencies(typedef: &ast::AltTypeDef) -> BTreeSet<String> {
    let mut deps = vec![];
    for (_name, typs) in & typedef.alts {
        for typ in typs {
            deps.extend(type_dependencies(typ).into_iter())
        }

    }
    deps.into_iter().collect()
}


fn fndef_dependencies(typedef: &ast::FnDef) -> BTreeSet<String> {
    let mut result = type_dependencies(&typedef.ret);
    let mut arguments = BTreeSet::new();
    for (name, typ) in &typedef.args {
        arguments.insert(name.to_string());
        result.extend(type_dependencies(typ).into_iter());
    }

    result.extend(expr_dependencies(&typedef.body, &arguments).into_iter());
    result
}

fn type_dependencies(typ: &ast::Type) -> BTreeSet<String> {
    match typ {
        ast::Type::Word(_n) => BTreeSet::new(),
        ast::Type::Vec(t, _n) => type_dependencies(t),
        ast::Type::Valid(t) => type_dependencies(t),
        ast::Type::TypeRef(r) => vec![r.to_string()].into_iter().collect(),
    }
}

fn expr_dependencies(expr: &ast::Expr, shadowed: &BTreeSet<String>) -> BTreeSet<String> {
    match expr {
        ast::Expr::Ident(_loc, ident) => {
            if shadowed.contains(&ident.to_string()) {
                BTreeSet::new()
            } else {
                vec![ident.to_string()].into_iter().collect()
            }
        },
        ast::Expr::Dot(_loc, e, _x) => expr_dependencies(e, shadowed),
        ast::Expr::Word(_loc, _w, _v) => BTreeSet::new(),
        ast::Expr::Enum(_loc, typ, _value) => type_dependencies(typ),
        ast::Expr::Struct(_loc, _fields) => BTreeSet::new(),
        ast::Expr::Vec(_loc, es) => {
            let mut results = BTreeSet::new();
            for e in es {
                results.extend(expr_dependencies(e, shadowed).into_iter());
            }
            results
        },
        ast::Expr::Call(_loc, func, es) => {
            let mut results = BTreeSet::new();
            const SPECIALS: &[&str] = &[
                "cat",
                "mux",
                "sext",
                "zext",
                "trycast",
                "word",
                "@Valid",
                "@Invalid",
            ];

            if !SPECIALS.contains(&func.as_str()) && !func.as_str().starts_with("@") {
                results.insert(func.to_string());
            }
            for e in es {
                results.extend(expr_dependencies(e, shadowed).into_iter());
            }
            results
        },
        ast::Expr::Let(_loc, x, type_ascription, e, b) => {
            let mut new_shadowed = shadowed.clone();
            new_shadowed.insert(x.to_string());

            let mut results = BTreeSet::new();
            if let Some(typ) = type_ascription {
                results.extend(type_dependencies(typ).into_iter());
            }
            results.extend(expr_dependencies(e, shadowed).into_iter());
            results.extend(expr_dependencies(b, &new_shadowed).into_iter());
            results
        },
        ast::Expr::UnOp(_loc, _op, e1) => expr_dependencies(e1, shadowed),
        ast::Expr::BinOp(_loc, _op, e1, e2) => {
            let mut results = BTreeSet::new();
            for e in &[e1, e2] {
                results.extend(expr_dependencies(e, shadowed).into_iter());
            }
            results
        },
        ast::Expr::If(_loc, c, e1, e2) => {
            let mut results = BTreeSet::new();
            for e in &[c, e1, e2] {
                results.extend(expr_dependencies(e, shadowed).into_iter());
            }
            results
        },
        ast::Expr::Match(_loc, e, arms) => {
            let mut results = BTreeSet::new();
            results.extend(expr_dependencies(e, shadowed).into_iter());
            for ast::MatchArm(pat, e) in arms {
                let mut new_shadowed = shadowed.clone();
                new_shadowed.extend(pat.bound_vars().into_iter());
                results.extend(expr_dependencies(e, &new_shadowed).into_iter());
            }
            results
        },
        ast::Expr::IdxField(_loc, e, _field) => expr_dependencies(e, shadowed),
        ast::Expr::Idx(_loc, e, _i) => expr_dependencies(e, shadowed),
        ast::Expr::IdxRange(_loc, e, _j, _i) => expr_dependencies(e, shadowed),
        ast::Expr::Hole(_loc, _name) => BTreeSet::new(),
    }
}

fn resolve_type(typ: &ast::Type, ctx: Context<String, Type>) -> Type {
    match typ {
        ast::Type::Word(n) => Type::word(*n),
        ast::Type::Vec(t, n) => Type::vec(resolve_type(t, ctx), *n),
        ast::Type::Valid(t) => Type::valid(resolve_type(t, ctx)),
        ast::Type::TypeRef(r) => ctx.lookup(&r.to_string()).expect(&format!("Couldn't find {r}")).clone(),
    }
}

fn resolve_struct_typedef(
    typedef: &ast::StructTypeDef,
    ctx: Context<String, Type>,
) -> Arc<StructTypeDef> {
    let mut fields: BTreeMap<String, Type> = BTreeMap::new();
    for (name, typ) in &typedef.fields {
        fields.insert(name.to_string(), resolve_type(typ, ctx.clone()));
    }

    let package_typedef = Arc::new(StructTypeDef {
        name: typedef.name.to_string(),
        fields: fields.into_iter().collect(),
    });
    package_typedef
}

fn resolve_enum_typedef(typedef: &ast::EnumTypeDef) -> Arc<EnumTypeDef> {
    let package_typedef = Arc::new(EnumTypeDef {
        name: typedef.name.to_string(),
        values: typedef.values.iter().map(|(name, val)| (name.to_string(), val.clone())).collect(),
    });
    package_typedef
}

fn resolve_alt_typedef(
    typedef: &ast::AltTypeDef,
    ctx: Context<String, Type>,
) -> Arc<AltTypeDef> {
    let mut alts: BTreeMap<String, Vec<Type>> = BTreeMap::new();
    for (name, typs) in &typedef.alts {
        let mut alt_types = vec![];
        for typ in typs {
            alt_types.push(resolve_type(typ, ctx.clone()));
        }
        alts.insert(name.to_string(), alt_types);
    }

    let package_typedef = Arc::new(AltTypeDef {
        name: typedef.name.to_string(),
        alts: alts.into_iter().collect(),
    });
    package_typedef
}

fn resolve_fndef(
    fndef: &ast::FnDef,
    ctx: Context<String, Type>,
    fn_ctx: Context<String, Arc<FnDef>>,
) -> Arc<FnDef> {
    let mut args: BTreeMap<String, Type> = BTreeMap::new();
    for (name, typ) in &fndef.args {
        args.insert(name.to_string(), resolve_type(typ, ctx.clone()));
    }

    let package_typedef = Arc::new(FnDef {
        name: fndef.name.to_string(),
        args: args.into_iter().collect(),
        ret: resolve_type(&fndef.ret, ctx.clone()),
        body: resolve_expr(&fndef.body, ctx.clone(), fn_ctx.clone()),
    });
    package_typedef
}

fn resolve_decls(
    decls: &[&ast::Decl],
    ctx: Context<String, Type>,
    mod_ctx: Context<String, Arc<Component>>,
    fndef_ctx: Context<String, Arc<FnDef>>,
) -> (Vec<Arc<Component>>, Vec<Wire>, Vec<When>) {
    let mut children = vec![];
    let mut wires = vec![];
    let mut whens = vec![];

    for decl in decls {
        match decl {
            ast::Decl::Mod(loc, name, decls) => {
                let (inner_children, wires, whens) = resolve_decls(
                    &decls.iter().collect::<Vec<_>>(),
                    ctx.clone(),
                    mod_ctx.clone(),
                    fndef_ctx.clone(),
                );
                let child = Component::Mod(loc.clone(), name.to_string(), inner_children, wires, whens);
                children.push(Arc::new(child));
            },
            ast::Decl::ModInst(loc, name, moddef_name) => {
                let child = Component::ModInst(
                    loc.clone(),
                    name.to_string(),
                    mod_ctx.lookup(&moddef_name.to_string()).unwrap(),
                );
                children.push(Arc::new(child));
            },
            ast::Decl::Incoming(loc, name, typ) => {
                let child =
                    Component::Incoming(loc.clone(), name.to_string(), resolve_type(typ, ctx.clone()));
                children.push(Arc::new(child));
            },
            ast::Decl::Outgoing(loc, name, typ) => {
                let child =
                    Component::Outgoing(loc.clone(), name.to_string(), resolve_type(typ, ctx.clone()));
                children.push(Arc::new(child));
            },
            ast::Decl::Node(loc, name, typ) => {
                let child =
                    Component::Node(loc.clone(), name.to_string(), resolve_type(typ, ctx.clone()));
                children.push(Arc::new(child));
            },
            ast::Decl::Reg(loc, name, typ, reset) => {
                let child = Component::Reg(
                    loc.clone(),
                    name.to_string(),
                    resolve_type(typ, ctx.clone()),
                    reset
                        .clone()
                        .map(|e| resolve_expr(&e, ctx.clone(), fndef_ctx.clone())),
                );
                children.push(Arc::new(child));
            },
            ast::Decl::Wire(loc, ast::Wire(_loc, target, expr, wire_type)) => {
                let wire = Wire(
                    loc.clone(),
                    target_to_path(target),
                    resolve_expr(expr, ctx.clone(), fndef_ctx.clone()),
                    wire_type.clone(),
                );
                wires.push(wire);
            },
            ast::Decl::When(_loc, ast::When(cond, wires)) => {
                let mut package_wires = vec![];
                let package_cond = resolve_expr(&*cond, ctx.clone(), fndef_ctx.clone());

                for ast::Wire(loc, target, expr, wire_type) in wires {
                    let package_wire = Wire(
                        loc.clone(),
                        target_to_path(target),
                        resolve_expr(expr, ctx.clone(), fndef_ctx.clone()),
                        wire_type.clone(),
                    );
                    package_wires.push(package_wire);
                }
                let package_when = When(package_cond, package_wires);
                whens.push(package_when);
            },
        }
    }

    (children, wires, whens)
}

fn resolve_moddef(
    moddef: &ast::ModDef,
    ctx: Context<String, Type>,
    mod_ctx: Context<String, Arc<Component>>,
    fndef_ctx: Context<String, Arc<FnDef>>,
) -> Arc<Component> {
    let ast::ModDef(loc, name, decls) = moddef;
    let decls_slice: &[&ast::Decl] = &decls.iter().collect::<Vec<_>>();
    let (children, wires, whens) = resolve_decls(decls_slice, ctx, mod_ctx, fndef_ctx.clone());
    Arc::new(Component::Mod(loc.clone(), name.to_string(), children, wires, whens))
}

fn resolve_extmoddef(
    moddef: &ast::ModDef,
    ctx: Context<String, Type>,
    mod_ctx: Context<String, Arc<Component>>,
    fndef_ctx: Context<String, Arc<FnDef>>,
) -> Arc<Component> {
    let ast::ModDef(loc, name, decls) = moddef;
    let decls_slice: &[&ast::Decl] = &decls.iter().collect::<Vec<_>>();
    let (children, wires, whens) = resolve_decls(decls_slice, ctx, mod_ctx, fndef_ctx.clone());
    assert!(wires.is_empty());
    assert!(whens.is_empty());
    Arc::new(Component::Ext(loc.clone(), name.to_string(), children))
}

fn resolve_expr(
    expr: &ast::Expr,
    ctx: Context<String, Type>,
    fndef_ctx: Context<String, Arc<FnDef>>,
) -> Arc<Expr> {
    Arc::new(match expr {
        ast::Expr::Ident(loc, id) => {
            Expr::Reference(loc.clone(), OnceCell::new(), id.to_string().into())
        },
        ast::Expr::Dot(loc, e, x) => {
            if let ast::Expr::Ident(_loc, id) = &**e {
                Expr::Reference(loc.clone(), OnceCell::new(), format!("{id}.{x}").into())
            } else {
                panic!()
            }
        },
        ast::Expr::Word(loc, w, n) => Expr::Word(loc.clone(), OnceCell::new(), *w, *n),
        ast::Expr::Enum(loc, typ, value) => {
            Expr::Enum(loc.clone(), OnceCell::new(), resolve_type(typ, ctx.clone()), value.clone())
        },
        ast::Expr::Struct(loc, fields) => {
            let package_fields = fields
                .into_iter()
                .map(|(name, expr)| {
                    (name.to_string(), resolve_expr(expr, ctx.clone(), fndef_ctx.clone()))
                })
                .collect();
            Expr::Struct(loc.clone(), OnceCell::new(), package_fields)
        },
        ast::Expr::Vec(loc, es) => {
            let package_es = es
                .into_iter()
                .map(|expr| resolve_expr(expr, ctx.clone(), fndef_ctx.clone()))
                .collect();
            Expr::Vec(loc.clone(), OnceCell::new(), package_es)
        },
        ast::Expr::Call(loc, func, es) => {
            let package_es = es
                .into_iter()
                .map(|expr| resolve_expr(expr, ctx.clone(), fndef_ctx.clone()))
                .collect();
            match func.as_str() {
                "cat" => Expr::Cat(loc.clone(), OnceCell::new(), package_es),
                "mux" => Expr::Mux(
                    loc.clone(),
                    OnceCell::new(),
                    package_es[0].clone(),
                    package_es[1].clone(),
                    package_es[2].clone(),
                ),
                "sext" => Expr::Sext(loc.clone(), OnceCell::new(), package_es[0].clone()),
                "zext" => Expr::Zext(loc.clone(), OnceCell::new(), package_es[0].clone()),
                "trycast" => Expr::TryCast(loc.clone(), OnceCell::new(), package_es[0].clone()),
                "word" => Expr::ToWord(loc.clone(), OnceCell::new(), package_es[0].clone()),
                "@Valid" => {
                    Expr::Ctor(loc.clone(), OnceCell::new(), "Valid".to_string(), package_es)
                },
                "@Invalid" => {
                    Expr::Ctor(loc.clone(), OnceCell::new(), "Invalid".to_string(), vec![])
                },
                fnname => {
                    let func = fnname.to_string();
                    if fnname.starts_with("@") {
                        let package_es = es
                            .into_iter()
                            .map(|expr| resolve_expr(expr, ctx.clone(), fndef_ctx.clone()))
                            .collect();
                        Expr::Ctor(loc.clone(), OnceCell::new(), fnname[1..].to_string(), package_es)
                    } else if let Some(fndef) = fndef_ctx.lookup(&func) {
                        let package_es = es
                            .into_iter()
                            .map(|expr| resolve_expr(expr, ctx.clone(), fndef_ctx.clone()))
                            .collect();
                        Expr::Call(loc.clone(), OnceCell::new(), fndef, package_es)
                    } else {
                        panic!("Unknown call: {func}")
                    }
                },
            }
        },
        ast::Expr::Let(loc, x, type_ascription, e, b) => {
            let package_e = resolve_expr(e, ctx.clone(), fndef_ctx.clone());
            let package_b = resolve_expr(b, ctx.clone(), fndef_ctx.clone());
            let package_ascription = type_ascription.clone().map(|typ| resolve_type(&typ, ctx.clone()));
            Expr::Let(loc.clone(), OnceCell::new(), x.to_string(), package_ascription, package_e, package_b)
        },
        ast::Expr::UnOp(loc, op, e1) => Expr::UnOp(
            loc.clone(),
            OnceCell::new(),
            *op,
            resolve_expr(&e1, ctx.clone(), fndef_ctx.clone()),
        ),
        ast::Expr::BinOp(loc, op, e1, e2) => Expr::BinOp(
            loc.clone(),
            OnceCell::new(),
            *op,
            resolve_expr(&e1, ctx.clone(), fndef_ctx.clone()),
            resolve_expr(&e2, ctx.clone(), fndef_ctx.clone()),
        ),
        ast::Expr::If(loc, c, e1, e2) => {
            let package_c = resolve_expr(c, ctx.clone(), fndef_ctx.clone());
            let package_e1 = resolve_expr(e1, ctx.clone(), fndef_ctx.clone());
            let package_e2 = resolve_expr(e2, ctx.clone(), fndef_ctx.clone());
            Expr::If(loc.clone(), OnceCell::new(), package_c, package_e1, package_e2)
        },
        ast::Expr::Match(loc, e, arms) => {
            let package_e = resolve_expr(e, ctx.clone(), fndef_ctx.clone());
            let package_arms = arms
                .into_iter()
                .map(|ast::MatchArm(pat, expr)| {
                    let package_expr = resolve_expr(expr, ctx.clone(), fndef_ctx.clone());
                    MatchArm(pat.clone(), package_expr)
                })
                .collect();
            Expr::Match(loc.clone(), OnceCell::new(), package_e, package_arms)
        },
        ast::Expr::IdxField(loc, e, field) => Expr::IdxField(
            loc.clone(),
            OnceCell::new(),
            resolve_expr(&e, ctx.clone(), fndef_ctx.clone()),
            field.to_string(),
        ),
        ast::Expr::Idx(loc, e, i) => Expr::Idx(
            loc.clone(),
            OnceCell::new(),
            resolve_expr(&e, ctx.clone(), fndef_ctx.clone()),
            *i,
        ),
        ast::Expr::IdxRange(loc, e, j, i) => Expr::IdxRange(
            loc.clone(),
            OnceCell::new(),
            resolve_expr(&e, ctx.clone(), fndef_ctx.clone()),
            *j,
            *i,
        ),
        ast::Expr::Hole(loc, name) => Expr::Hole(loc.clone(), OnceCell::new(), name.clone().map(|name| name.to_string())),
    })
}

fn target_to_path(target: &ast::Target) -> Path {
    match target {
        ast::Target::Local(id) => id.as_str().into(),
        ast::Target::Nonlocal(id1, id2) => format!("{}.{}", id1.as_str(), id2.as_str()).into(),
    }
}
