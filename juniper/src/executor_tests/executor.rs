mod field_execution {
    use value::Value;
    use ast::InputValue;
    use schema::model::RootNode;
    use types::scalars::EmptyMutation;

    struct DataType;
    struct DeepDataType;

    graphql_object!(DataType: () |&self| {
        field a() -> &str { "Apple" }
        field b() -> &str { "Banana" }
        field c() -> &str { "Cookie" }
        field d() -> &str { "Donut" }
        field e() -> &str { "Egg" }
        field f() -> &str { "Fish" }

        field pic(size: Option<i32>) -> String {
            format!("Pic of size: {}", size.unwrap_or(50))
        }

        field deep() -> DeepDataType {
            DeepDataType
        }
    });

    graphql_object!(DeepDataType: () |&self| {
        field a() -> &str { "Already Been Done" }
        field b() -> &str { "Boring" }
        field c() -> Vec<Option<&str>> { vec![Some("Contrived"), None, Some("Confusing")] }

        field deeper() -> Vec<Option<DataType>> { vec![Some(DataType), None, Some(DataType) ] }
    });

    #[test]
    fn test() {
        let schema = RootNode::new(DataType, EmptyMutation::<()>::new());
        let doc = r"
          query Example($size: Int) {
            a,
            b,
            x: c
            ...c
            f
            ...on DataType {
              pic(size: $size)
            }
            deep {
              a
              b
              c
              deeper {
                a
                b
              }
            }
          }

          fragment c on DataType {
            d
            e
          }";

        let vars = vec![
            ("size".to_owned(), InputValue::int(100))
        ].into_iter()
            .collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        assert_eq!(errs, []);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("a", Value::string("Apple")),
                ("b", Value::string("Banana")),
                ("x", Value::string("Cookie")),
                ("d", Value::string("Donut")),
                ("e", Value::string("Egg")),
                ("f", Value::string("Fish")),
                ("pic", Value::string("Pic of size: 100")),
                ("deep", Value::object(vec![
                    ("a", Value::string("Already Been Done")),
                    ("b", Value::string("Boring")),
                    ("c", Value::list(vec![
                        Value::string("Contrived"),
                        Value::null(),
                        Value::string("Confusing"),
                    ])),
                    ("deeper", Value::list(vec![
                        Value::object(vec![
                            ("a", Value::string("Apple")),
                            ("b", Value::string("Banana")),
                        ].into_iter().collect()),
                        Value::null(),
                        Value::object(vec![
                            ("a", Value::string("Apple")),
                            ("b", Value::string("Banana")),
                        ].into_iter().collect()),
                    ])),
                ].into_iter().collect())),
            ].into_iter().collect()));
    }
}


mod merge_parallel_fragments {
    use value::Value;
    use schema::model::RootNode;
    use types::scalars::EmptyMutation;

    struct Type;

    graphql_object!(Type: () |&self| {
        field a() -> &str { "Apple" }
        field b() -> &str { "Banana" }
        field c() -> &str { "Cherry" }
        field deep() -> Type { Type }
    });

    #[test]
    fn test() {
        let schema = RootNode::new(Type, EmptyMutation::<()>::new());
        let doc = r"
          { a, ...FragOne, ...FragTwo }
          fragment FragOne on Type {
            b
            deep { b, deeper: deep { b } }
          }
          fragment FragTwo on Type {
            c
            deep { c, deeper: deep { c } }
          }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        assert_eq!(errs, []);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("a", Value::string("Apple")),
                ("b", Value::string("Banana")),
                ("deep", Value::object(vec![
                    ("b", Value::string("Banana")),
                    ("deeper", Value::object(vec![
                        ("b", Value::string("Banana")),
                        ("c", Value::string("Cherry")),
                    ].into_iter().collect())),
                    ("c", Value::string("Cherry")),
                ].into_iter().collect())),
                ("c", Value::string("Cherry")),
            ].into_iter().collect()));
    }
}

mod threads_context_correctly {
    use value::Value;
    use types::scalars::EmptyMutation;
    use schema::model::RootNode;
    use executor::Context;

    struct Schema;

    struct TestContext {
        value: String,
    }

    impl Context for TestContext {}

    graphql_object!(Schema: TestContext |&self| {
        field a(&executor) -> String { executor.context().value.clone() }
    });

    #[test]
    fn test() {
        let schema = RootNode::new(Schema, EmptyMutation::<TestContext>::new());
        let doc = r"{ a }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(
            doc,
            None,
            &schema,
            &vars,
            &TestContext {
                value: "Context value".to_owned(),
            },
        ).expect("Execution failed");

        assert_eq!(errs, []);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("a", Value::string("Context value")),
            ].into_iter().collect()));
    }
}

mod dynamic_context_switching {
    use ordermap::OrderMap;

    use value::Value;
    use types::scalars::EmptyMutation;
    use schema::model::RootNode;
    use parser::SourcePosition;
    use executor::{Context, ExecutionError, FieldError, FieldResult};
    use result_ext::ResultExt;

    struct Schema;

    struct InnerContext {
        value: String,
    }

    struct OuterContext {
        items: OrderMap<i32, InnerContext>,
    }

    impl Context for OuterContext {}
    impl Context for InnerContext {}

    struct ItemRef;

    graphql_object!(Schema: OuterContext |&self| {
        field item_opt(&executor, key: i32) -> Option<(&InnerContext, ItemRef)> {
            executor.context().items.get(&key).map(|c| (c, ItemRef))
        }

        field item_res(&executor, key: i32) -> FieldResult<(&InnerContext, ItemRef)> {
            executor.context().items.get(&key)
                .ok_or(format!("Could not find key {}", key))
                .map(|c| (c, ItemRef))
                .to_field_result()
        }

        field item_res_opt(&executor, key: i32) -> FieldResult<Option<(&InnerContext, ItemRef)>> {
            if key > 100 {
                Err(format!("Key too large: {}", key)).to_field_result()
            } else {
                Ok(executor.context().items.get(&key)
                   .map(|c| (c, ItemRef)))
            }
        }

        field item_always(&executor, key: i32) -> (&InnerContext, ItemRef) {
            executor.context().items.get(&key)
                .map(|c| (c, ItemRef))
                .unwrap()
        }
    });

    graphql_object!(ItemRef: InnerContext |&self| {
        field value(&executor) -> String { executor.context().value.clone() }
    });

    #[test]
    fn test_opt() {
        let schema = RootNode::new(Schema, EmptyMutation::<OuterContext>::new());
        let doc = r"{ first: itemOpt(key: 0) { value }, missing: itemOpt(key: 2) { value } }";

        let vars = vec![].into_iter().collect();

        let ctx = OuterContext {
            items: vec![
                (0, InnerContext { value: "First value".to_owned() }),
                (1, InnerContext { value: "Second value".to_owned() }),
            ].into_iter()
                .collect(),
        };

        let (result, errs) = ::execute(doc, None, &schema, &vars, &ctx).expect("Execution failed");

        assert_eq!(errs, []);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("first", Value::object(vec![
                    ("value", Value::string("First value")),
                ].into_iter().collect())),
                ("missing", Value::null()),
            ].into_iter().collect()));
    }

    #[test]
    fn test_res_success() {
        let schema = RootNode::new(Schema, EmptyMutation::<OuterContext>::new());
        let doc = r"
          {
            first: itemRes(key: 0) { value }
          }
          ";

        let vars = vec![].into_iter().collect();

        let ctx = OuterContext {
            items: vec![
                (0, InnerContext { value: "First value".to_owned() }),
                (1, InnerContext { value: "Second value".to_owned() }),
            ].into_iter()
                .collect(),
        };

        let (result, errs) = ::execute(doc, None, &schema, &vars, &ctx).expect("Execution failed");

        assert_eq!(errs, vec![]);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("first", Value::object(vec![
                    ("value", Value::string("First value")),
                ].into_iter().collect())),
            ].into_iter().collect()));
    }

    #[test]
    fn test_res_fail() {
        let schema = RootNode::new(Schema, EmptyMutation::<OuterContext>::new());
        let doc = r"
          {
            missing: itemRes(key: 2) { value }
          }
          ";

        let vars = vec![].into_iter().collect();

        let ctx = OuterContext {
            items: vec![
                (0, InnerContext { value: "First value".to_owned() }),
                (1, InnerContext { value: "Second value".to_owned() }),
            ].into_iter()
                .collect(),
        };

        let (result, errs) = ::execute(doc, None, &schema, &vars, &ctx).expect("Execution failed");

        assert_eq!(errs, vec![
            ExecutionError::new(
                SourcePosition::new(25, 2, 12),
                &["missing"],
                FieldError::new("Could not find key 2", Value::null()),
            ),
        ]);

        println!("Result: {:?}", result);

        assert_eq!(result, Value::null());
    }

    #[test]
    fn test_res_opt() {
        let schema = RootNode::new(Schema, EmptyMutation::<OuterContext>::new());
        let doc = r"
          {
            first: itemResOpt(key: 0) { value }
            missing: itemResOpt(key: 2) { value }
            tooLarge: itemResOpt(key: 200) { value }
          }
          ";

        let vars = vec![].into_iter().collect();

        let ctx = OuterContext {
            items: vec![
                (0, InnerContext { value: "First value".to_owned() }),
                (1, InnerContext { value: "Second value".to_owned() }),
            ].into_iter()
                .collect(),
        };

        let (result, errs) = ::execute(doc, None, &schema, &vars, &ctx).expect("Execution failed");

        assert_eq!(errs, [
            ExecutionError::new(
                SourcePosition::new(123, 4, 12),
                &["tooLarge"],
                FieldError::new("Key too large: 200", Value::null()),
            ),
        ]);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("first", Value::object(vec![
                    ("value", Value::string("First value")),
                ].into_iter().collect())),
                ("missing", Value::null()),
                ("tooLarge", Value::null()),
            ].into_iter().collect()));
    }

    #[test]
    fn test_always() {
        let schema = RootNode::new(Schema, EmptyMutation::<OuterContext>::new());
        let doc = r"{ first: itemAlways(key: 0) { value } }";

        let vars = vec![].into_iter().collect();

        let ctx = OuterContext {
            items: vec![
                (0, InnerContext { value: "First value".to_owned() }),
                (1, InnerContext { value: "Second value".to_owned() }),
            ].into_iter()
                .collect(),
        };

        let (result, errs) = ::execute(doc, None, &schema, &vars, &ctx).expect("Execution failed");

        assert_eq!(errs, []);

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            Value::object(vec![
                ("first", Value::object(vec![
                    ("value", Value::string("First value")),
                ].into_iter().collect())),
            ].into_iter().collect()));
    }
}

mod propagates_errors_to_nullable_fields {
    use value::Value;
    use schema::model::RootNode;
    use executor::{ExecutionError, FieldError, FieldResult};
    use parser::SourcePosition;
    use types::scalars::EmptyMutation;

    struct Schema;
    struct Inner;

    graphql_object!(Schema: () |&self| {
        field inner() -> Inner { Inner }
        field inners() -> Vec<Inner> { (0..5).map(|_| Inner).collect() }
        field nullable_inners() -> Vec<Option<Inner>> { (0..5).map(|_| Some(Inner)).collect() }
    });

    graphql_object!(Inner: () |&self| {
        field nullable_field() -> Option<Inner> { Some(Inner) }
        field non_nullable_field() -> Inner { Inner }
        field nullable_error_field() -> FieldResult<Option<&str>> { Err("Error for nullableErrorField")? }
        field non_nullable_error_field() -> FieldResult<&str> { Err("Error for nonNullableErrorField")? }
    });

    #[test]
    fn nullable_first_level() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inner { nullableErrorField } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!({ "inner": { "nullableErrorField": None } }));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(10, 0, 10),
                    &["inner", "nullableErrorField"],
                    FieldError::new("Error for nullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn non_nullable_first_level() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inner { nonNullableErrorField } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!(None));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(10, 0, 10),
                    &["inner", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn nullable_nested_level() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inner { nullableField { nonNullableErrorField } } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!({ "inner": { "nullableField": None } }));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(26, 0, 26),
                    &["inner", "nullableField", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn non_nullable_nested_level() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inner { nonNullableField { nonNullableErrorField } } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!(None));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(29, 0, 29),
                    &["inner", "nonNullableField", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn nullable_innermost() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inner { nonNullableField { nullableErrorField } } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!({ "inner": { "nonNullableField": { "nullableErrorField": None } } }));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(29, 0, 29),
                    &["inner", "nonNullableField", "nullableErrorField"],
                    FieldError::new("Error for nullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn non_null_list() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ inners { nonNullableErrorField } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!(None));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(11, 0, 11),
                    &["inners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
            ]);
    }

    #[test]
    fn non_null_list_of_nullable() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ nullableInners { nonNullableErrorField } }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        println!("Result: {:?}", result);

        assert_eq!(
            result,
            graphql_value!({ "nullableInners": [None, None, None, None, None] }));

        assert_eq!(
            errs,
            vec![
                ExecutionError::new(
                    SourcePosition::new(19, 0, 19),
                    &["nullableInners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
                ExecutionError::new(
                    SourcePosition::new(19, 0, 19),
                    &["nullableInners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
                ExecutionError::new(
                    SourcePosition::new(19, 0, 19),
                    &["nullableInners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
                ExecutionError::new(
                    SourcePosition::new(19, 0, 19),
                    &["nullableInners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
                ExecutionError::new(
                    SourcePosition::new(19, 0, 19),
                    &["nullableInners", "nonNullableErrorField"],
                    FieldError::new("Error for nonNullableErrorField", Value::null()),
                ),
            ]);
    }
}

mod named_operations {
    use value::Value;
    use schema::model::RootNode;
    use types::scalars::EmptyMutation;
    use GraphQLError;

    struct Schema;

    graphql_object!(Schema: () |&self| {
        field a() -> &str { "b" }
    });

    #[test]
    fn uses_inline_operation_if_no_name_provided() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"{ a }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        assert_eq!(errs, []);

        assert_eq!(
            result,
            Value::object(vec![
                ("a", Value::string("b")),
            ].into_iter().collect()));
    }

    #[test]
    fn uses_only_named_operation() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"query Example { a }";

        let vars = vec![].into_iter().collect();

        let (result, errs) = ::execute(doc, None, &schema, &vars, &()).expect("Execution failed");

        assert_eq!(errs, []);

        assert_eq!(
            result,
            Value::object(vec![
                ("a", Value::string("b")),
            ].into_iter().collect()));
    }

    #[test]
    fn uses_named_operation_if_name_provided() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"query Example { first: a } query OtherExample { second: a }";

        let vars = vec![].into_iter().collect();

        let (result, errs) =
            ::execute(doc, Some("OtherExample"), &schema, &vars, &()).expect("Execution failed");

        assert_eq!(errs, []);

        assert_eq!(
            result,
            Value::object(vec![
                ("second", Value::string("b")),
            ].into_iter().collect()));
    }

    #[test]
    fn error_if_multiple_operations_provided_but_no_name() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"query Example { first: a } query OtherExample { second: a }";

        let vars = vec![].into_iter().collect();

        let err = ::execute(doc, None, &schema, &vars, &()).unwrap_err();

        assert_eq!(err, GraphQLError::MultipleOperationsProvided);
    }

    #[test]
    fn error_if_unknown_operation_name_provided() {
        let schema = RootNode::new(Schema, EmptyMutation::<()>::new());
        let doc = r"query Example { first: a } query OtherExample { second: a }";

        let vars = vec![].into_iter().collect();

        let err = ::execute(doc, Some("UnknownExample"), &schema, &vars, &()).unwrap_err();

        assert_eq!(err, GraphQLError::UnknownOperationName);
    }
}
