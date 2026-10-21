use std::cmp::Ordering;

use crate::condition::eval_context::ConditionEvalContext;
use crate::condition::js_coercion::js_compare;
use crate::extensions::FindGrowthBookAttribute;
use crate::model_public::{GrowthBookAttribute, GrowthBookAttributeValue};

pub struct OrderComparison;

impl OrderComparison {
    pub fn gt(
        parent_attribute: Option<&GrowthBookAttribute>,
        feature_attribute: &GrowthBookAttribute,
        ctx: &ConditionEvalContext,
        array_size: bool,
    ) -> bool {
        evaluate(parent_attribute, feature_attribute, ctx, array_size, Ordering::is_gt)
    }

    pub fn gte(
        parent_attribute: Option<&GrowthBookAttribute>,
        feature_attribute: &GrowthBookAttribute,
        ctx: &ConditionEvalContext,
        array_size: bool,
    ) -> bool {
        evaluate(parent_attribute, feature_attribute, ctx, array_size, Ordering::is_ge)
    }

    pub fn lt(
        parent_attribute: Option<&GrowthBookAttribute>,
        feature_attribute: &GrowthBookAttribute,
        ctx: &ConditionEvalContext,
        array_size: bool,
    ) -> bool {
        evaluate(parent_attribute, feature_attribute, ctx, array_size, Ordering::is_lt)
    }

    pub fn lte(
        parent_attribute: Option<&GrowthBookAttribute>,
        feature_attribute: &GrowthBookAttribute,
        ctx: &ConditionEvalContext,
        array_size: bool,
    ) -> bool {
        evaluate(parent_attribute, feature_attribute, ctx, array_size, Ordering::is_le)
    }
}

fn evaluate(
    parent_attribute: Option<&GrowthBookAttribute>,
    feature_attribute: &GrowthBookAttribute,
    ctx: &ConditionEvalContext,
    array_size: bool,
    condition: fn(Ordering) -> bool,
) -> bool {
    // JS getPath returns null for missing paths; the operators then use JS `<`/`>`.
    let user_value = match ctx.find_value(&parent_attribute.unwrap_or(feature_attribute).key).unwrap_or(GrowthBookAttributeValue::Empty) {
        GrowthBookAttributeValue::Array(items) if array_size => GrowthBookAttributeValue::Int(items.len() as i64),
        it => it,
    };
    js_compare(&user_value, &feature_attribute.value).is_some_and(condition)
}
