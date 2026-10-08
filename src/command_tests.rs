use super::*;
use crate::scene::Element;

#[derive(Debug, Clone, PartialEq, Default)]
struct World {
    scene: Scene,
    names: Names,
}

fn run(world: &mut World, line: &str) -> Result<Option<ElementId>, CommandError> {
    let command = line.parse::<Command>().expect("parses");
    command.apply(&mut world.scene, &mut world.names)
}

#[test]
fn rectangle_is_rect() {
    assert_eq!(
        "rectangle in 2".parse::<Command>(),
        Ok(Command::AddRect {
            parent: Some(Target::Id(ElementId(2)))
        })
    );
}

#[test]
fn ids_up_to_ten_are_named_by_their_word() {
    let mut world = World::default();
    assert_eq!(
        run(&mut world, "text in three hi"),
        Err(CommandError::Scene(SceneError::Missing(ElementId(3))))
    );
    for _ in 0..11 {
        run(&mut world, "rect").unwrap();
    }
    assert_eq!(world.names.of(ElementId(1)), ["one"]);
    assert!(world.names.of(ElementId(11)).is_empty());
    assert_eq!(run(&mut world, "text in three hi"), Ok(Some(ElementId(12))));
    assert_eq!(run(&mut world, "remove ten"), Ok(None));
    assert_eq!(
        run(&mut world, "remove eleven"),
        Err(CommandError::UnknownName("eleven".into()))
    );
}

#[test]
fn ids_and_names_can_follow_number() {
    assert_eq!(
        "text in number two hi".parse::<Command>(),
        Ok(Command::AddText {
            parent: Some(Target::Name("two".into())),
            text: "hi".into()
        })
    );
    assert_eq!(
        "remove number 4".parse::<Command>(),
        Ok(Command::Remove {
            id: Target::Id(ElementId(4))
        })
    );
    assert!("remove number".parse::<Command>().is_err());
}

#[test]
fn names_work_wherever_ids_do() {
    let mut world = World::default();
    run(&mut world, "rect").unwrap();
    assert_eq!(run(&mut world, "name 1 test"), Ok(None));
    let text = run(&mut world, "text in test hello").unwrap().unwrap();
    let inner = run(&mut world, "rect in test").unwrap().unwrap();
    assert_eq!(
        world.scene.get(ElementId(1)),
        Some(&Element::Rect {
            children: vec![text, inner]
        })
    );

    run(&mut world, "name 2 greeting").unwrap();
    run(&mut world, "set greeting bye").unwrap();
    assert_eq!(
        world.scene.get(text),
        Some(&Element::Text { text: "bye".into() })
    );

    run(&mut world, "remove test").unwrap();
    assert!(world.scene.roots().is_empty());
}

#[test]
fn names_ignore_case() {
    let mut world = World::default();
    run(&mut world, "rect").unwrap();
    run(&mut world, "name 1 Test").unwrap();
    assert_eq!(world.names.of(ElementId(1)), ["one", "test"]);
    assert_eq!(run(&mut world, "text in TEST hi"), Ok(Some(ElementId(2))));
    // Not lowercased by parsing this time.
    let remove = Command::Remove {
        id: Target::Name("tEsT".into()),
    };
    assert_eq!(remove.apply(&mut world.scene, &mut world.names), Ok(None));
    assert!(world.scene.roots().is_empty());
}

#[test]
fn an_element_can_have_several_names() {
    let mut world = World::default();
    run(&mut world, "text hi").unwrap();
    run(&mut world, "name 1 zed").unwrap();
    run(&mut world, "name zed alpha").unwrap();
    assert_eq!(world.names.of(ElementId(1)), ["alpha", "one", "zed"]);
}

#[test]
fn naming_again_moves_the_name() {
    let mut world = World::default();
    run(&mut world, "text a").unwrap();
    run(&mut world, "text b").unwrap();
    run(&mut world, "name 1 first").unwrap();
    run(&mut world, "name 1 test").unwrap();
    run(&mut world, "name 2 test").unwrap();
    assert_eq!(world.names.of(ElementId(1)), ["first", "one"]);
    assert_eq!(world.names.of(ElementId(2)), ["test", "two"]);
    run(&mut world, "set test changed").unwrap();
    assert_eq!(
        world.scene.get(ElementId(2)),
        Some(&Element::Text {
            text: "changed".into()
        })
    );
}

#[test]
fn remove_drops_the_names_of_everything_removed() {
    let mut world = World::default();
    run(&mut world, "rect").unwrap();
    run(&mut world, "rect in 1").unwrap();
    run(&mut world, "text in 2 deep").unwrap();
    run(&mut world, "text kept").unwrap();
    for line in ["name 1 outer", "name 2 inner", "name 3 deep", "name 4 kept"] {
        run(&mut world, line).unwrap();
    }
    run(&mut world, "remove outer").unwrap();
    for name in ["outer", "inner", "deep", "three"] {
        assert_eq!(
            run(&mut world, &format!("set {name} x")),
            Err(CommandError::UnknownName(name.into()))
        );
    }
    assert!(world.names.of(ElementId(3)).is_empty());
    assert_eq!(world.names.of(ElementId(4)), ["four", "kept"]);
}

#[test]
fn clear_drops_names() {
    let mut world = World::default();
    run(&mut world, "text hi").unwrap();
    run(&mut world, "name 1 greeting").unwrap();
    run(&mut world, "clear").unwrap();
    run(&mut world, "text again").unwrap();
    assert_eq!(
        run(&mut world, "set greeting x"),
        Err(CommandError::UnknownName("greeting".into()))
    );
    // The number words come back.
    assert_eq!(run(&mut world, "set two x"), Ok(None));
}

#[test]
fn unknown_names_and_missing_elements_change_nothing() {
    let mut world = World::default();
    run(&mut world, "rect").unwrap();
    run(&mut world, "name 1 box").unwrap();
    let before = world.clone();
    assert_eq!(
        run(&mut world, "remove x"),
        Err(CommandError::UnknownName("x".into()))
    );
    assert_eq!(
        run(&mut world, "text in nowhere hi"),
        Err(CommandError::UnknownName("nowhere".into()))
    );
    assert_eq!(
        run(&mut world, "name nowhere lid"),
        Err(CommandError::UnknownName("nowhere".into()))
    );
    // Would move "box" if it went through.
    assert_eq!(
        run(&mut world, "name 9 box"),
        Err(CommandError::Scene(SceneError::Missing(ElementId(9))))
    );
    assert_eq!(world, before);
}

#[test]
fn name_parses_one_lowercase_word() {
    assert_eq!(
        "name 3 Box".parse::<Command>(),
        Ok(Command::Name {
            target: Target::Id(ElementId(3)),
            name: "box".into()
        })
    );
    assert_eq!(
        "name box lid".parse::<Command>(),
        Ok(Command::Name {
            target: Target::Name("box".into()),
            name: "lid".into()
        })
    );
}

#[test]
fn unname_removes_one_name() {
    let mut world = World::default();
    run(&mut world, "text hi").unwrap();
    run(&mut world, "name 1 a").unwrap();
    run(&mut world, "name 1 b").unwrap();
    assert_eq!(run(&mut world, "unname a"), Ok(None));
    assert_eq!(world.names.of(ElementId(1)), ["b", "one"]);
    assert_eq!(run(&mut world, "set b bye"), Ok(None));
    assert_eq!(
        run(&mut world, "set a x"),
        Err(CommandError::UnknownName("a".into()))
    );
    assert_eq!(
        world.scene.get(ElementId(1)),
        Some(&Element::Text { text: "bye".into() })
    );
}

#[test]
fn unname_of_an_unknown_name_changes_nothing() {
    let mut world = World::default();
    run(&mut world, "text hi").unwrap();
    run(&mut world, "name 1 a").unwrap();
    let before = world.clone();
    assert_eq!(
        run(&mut world, "unname b"),
        Err(CommandError::UnknownName("b".into()))
    );
    assert_eq!(world, before);
}

#[test]
fn unname_all_leaves_only_the_number_words_and_keeps_the_elements() {
    let mut world = World::default();
    run(&mut world, "rect").unwrap();
    run(&mut world, "text in 1 hi").unwrap();
    for line in ["name 1 box", "name 2 greeting", "name 2 hello"] {
        run(&mut world, line).unwrap();
    }
    let before = world.clone();
    assert_eq!(run(&mut world, "unname all"), Ok(None));
    assert_eq!(world.scene.roots(), before.scene.roots());
    for id in [ElementId(1), ElementId(2)] {
        assert_eq!(world.scene.get(id), before.scene.get(id));
    }
    assert_eq!(world.names.of(ElementId(1)), ["one"]);
    assert_eq!(world.names.of(ElementId(2)), ["two"]);
    assert_eq!(
        run(&mut world, "remove box"),
        Err(CommandError::UnknownName("box".into()))
    );
}

#[test]
fn unname_all_without_names_succeeds() {
    let mut world = World::default();
    assert_eq!(run(&mut world, "unname all"), Ok(None));
    assert_eq!(world, World::default());
}

#[test]
fn unname_ignores_case() {
    let mut world = World::default();
    run(&mut world, "text hi").unwrap();
    run(&mut world, "name 1 test").unwrap();
    assert_eq!(run(&mut world, "unname Test"), Ok(None));
    assert_eq!(world.names.of(ElementId(1)), ["one"]);
    assert_eq!("unname ALL".parse::<Command>(), Ok(Command::UnnameAll));
}

#[test]
fn rejects_bad_input() {
    assert!("".parse::<Command>().is_err());
    assert!("jump".parse::<Command>().is_err());
    assert!("rect in".parse::<Command>().is_err());
    assert!("clear now".parse::<Command>().is_err());
    assert!("name 1".parse::<Command>().is_err());
    assert!("name 1 in".parse::<Command>().is_err());
    assert!("name 1 number".parse::<Command>().is_err());
    assert!("name 1 all".parse::<Command>().is_err());
    assert!("name 1 a b".parse::<Command>().is_err());
    assert!("unname".parse::<Command>().is_err());
    assert!("unname 3".parse::<Command>().is_err());
    assert!("unname a b".parse::<Command>().is_err());
}
