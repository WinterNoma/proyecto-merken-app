// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'api.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$PulseTarget {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PulseTarget);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'PulseTarget()';
}


}

/// @nodoc
class $PulseTargetCopyWith<$Res>  {
$PulseTargetCopyWith(PulseTarget _, $Res Function(PulseTarget) __);
}


/// Adds pattern-matching-related methods to [PulseTarget].
extension PulseTargetPatterns on PulseTarget {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( PulseTarget_Local value)?  local,TResult Function( PulseTarget_Demo value)?  demo,required TResult orElse(),}){
final _that = this;
switch (_that) {
case PulseTarget_Local() when local != null:
return local(_that);case PulseTarget_Demo() when demo != null:
return demo(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( PulseTarget_Local value)  local,required TResult Function( PulseTarget_Demo value)  demo,}){
final _that = this;
switch (_that) {
case PulseTarget_Local():
return local(_that);case PulseTarget_Demo():
return demo(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( PulseTarget_Local value)?  local,TResult? Function( PulseTarget_Demo value)?  demo,}){
final _that = this;
switch (_that) {
case PulseTarget_Local() when local != null:
return local(_that);case PulseTarget_Demo() when demo != null:
return demo(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String storeId,  String? astronautId)?  local,TResult Function( DemoTarget field0)?  demo,required TResult orElse(),}) {final _that = this;
switch (_that) {
case PulseTarget_Local() when local != null:
return local(_that.storeId,_that.astronautId);case PulseTarget_Demo() when demo != null:
return demo(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String storeId,  String? astronautId)  local,required TResult Function( DemoTarget field0)  demo,}) {final _that = this;
switch (_that) {
case PulseTarget_Local():
return local(_that.storeId,_that.astronautId);case PulseTarget_Demo():
return demo(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String storeId,  String? astronautId)?  local,TResult? Function( DemoTarget field0)?  demo,}) {final _that = this;
switch (_that) {
case PulseTarget_Local() when local != null:
return local(_that.storeId,_that.astronautId);case PulseTarget_Demo() when demo != null:
return demo(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class PulseTarget_Local extends PulseTarget {
  const PulseTarget_Local({required this.storeId, this.astronautId}): super._();
  

 final  String storeId;
 final  String? astronautId;

/// Create a copy of PulseTarget
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PulseTarget_LocalCopyWith<PulseTarget_Local> get copyWith => _$PulseTarget_LocalCopyWithImpl<PulseTarget_Local>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PulseTarget_Local&&(identical(other.storeId, storeId) || other.storeId == storeId)&&(identical(other.astronautId, astronautId) || other.astronautId == astronautId));
}


@override
int get hashCode => Object.hash(runtimeType,storeId,astronautId);

@override
String toString() {
  return 'PulseTarget.local(storeId: $storeId, astronautId: $astronautId)';
}


}

/// @nodoc
abstract mixin class $PulseTarget_LocalCopyWith<$Res> implements $PulseTargetCopyWith<$Res> {
  factory $PulseTarget_LocalCopyWith(PulseTarget_Local value, $Res Function(PulseTarget_Local) _then) = _$PulseTarget_LocalCopyWithImpl;
@useResult
$Res call({
 String storeId, String? astronautId
});




}
/// @nodoc
class _$PulseTarget_LocalCopyWithImpl<$Res>
    implements $PulseTarget_LocalCopyWith<$Res> {
  _$PulseTarget_LocalCopyWithImpl(this._self, this._then);

  final PulseTarget_Local _self;
  final $Res Function(PulseTarget_Local) _then;

/// Create a copy of PulseTarget
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? storeId = null,Object? astronautId = freezed,}) {
  return _then(PulseTarget_Local(
storeId: null == storeId ? _self.storeId : storeId // ignore: cast_nullable_to_non_nullable
as String,astronautId: freezed == astronautId ? _self.astronautId : astronautId // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class PulseTarget_Demo extends PulseTarget {
  const PulseTarget_Demo(this.field0): super._();
  

 final  DemoTarget field0;

/// Create a copy of PulseTarget
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PulseTarget_DemoCopyWith<PulseTarget_Demo> get copyWith => _$PulseTarget_DemoCopyWithImpl<PulseTarget_Demo>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PulseTarget_Demo&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'PulseTarget.demo(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $PulseTarget_DemoCopyWith<$Res> implements $PulseTargetCopyWith<$Res> {
  factory $PulseTarget_DemoCopyWith(PulseTarget_Demo value, $Res Function(PulseTarget_Demo) _then) = _$PulseTarget_DemoCopyWithImpl;
@useResult
$Res call({
 DemoTarget field0
});




}
/// @nodoc
class _$PulseTarget_DemoCopyWithImpl<$Res>
    implements $PulseTarget_DemoCopyWith<$Res> {
  _$PulseTarget_DemoCopyWithImpl(this._self, this._then);

  final PulseTarget_Demo _self;
  final $Res Function(PulseTarget_Demo) _then;

/// Create a copy of PulseTarget
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(PulseTarget_Demo(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as DemoTarget,
  ));
}


}

/// @nodoc
mixin _$SaveOutcome {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SaveOutcome);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SaveOutcome()';
}


}

/// @nodoc
class $SaveOutcomeCopyWith<$Res>  {
$SaveOutcomeCopyWith(SaveOutcome _, $Res Function(SaveOutcome) __);
}


/// Adds pattern-matching-related methods to [SaveOutcome].
extension SaveOutcomePatterns on SaveOutcome {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SaveOutcome_NeedsConfirmation value)?  needsConfirmation,TResult Function( SaveOutcome_Saved value)?  saved,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation() when needsConfirmation != null:
return needsConfirmation(_that);case SaveOutcome_Saved() when saved != null:
return saved(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SaveOutcome_NeedsConfirmation value)  needsConfirmation,required TResult Function( SaveOutcome_Saved value)  saved,}){
final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation():
return needsConfirmation(_that);case SaveOutcome_Saved():
return saved(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SaveOutcome_NeedsConfirmation value)?  needsConfirmation,TResult? Function( SaveOutcome_Saved value)?  saved,}){
final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation() when needsConfirmation != null:
return needsConfirmation(_that);case SaveOutcome_Saved() when saved != null:
return saved(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( PreparedPulse prepared)?  needsConfirmation,TResult Function( PulseReceipt receipt)?  saved,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation() when needsConfirmation != null:
return needsConfirmation(_that.prepared);case SaveOutcome_Saved() when saved != null:
return saved(_that.receipt);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( PreparedPulse prepared)  needsConfirmation,required TResult Function( PulseReceipt receipt)  saved,}) {final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation():
return needsConfirmation(_that.prepared);case SaveOutcome_Saved():
return saved(_that.receipt);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( PreparedPulse prepared)?  needsConfirmation,TResult? Function( PulseReceipt receipt)?  saved,}) {final _that = this;
switch (_that) {
case SaveOutcome_NeedsConfirmation() when needsConfirmation != null:
return needsConfirmation(_that.prepared);case SaveOutcome_Saved() when saved != null:
return saved(_that.receipt);case _:
  return null;

}
}

}

/// @nodoc


class SaveOutcome_NeedsConfirmation extends SaveOutcome {
  const SaveOutcome_NeedsConfirmation({required this.prepared}): super._();
  

 final  PreparedPulse prepared;

/// Create a copy of SaveOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SaveOutcome_NeedsConfirmationCopyWith<SaveOutcome_NeedsConfirmation> get copyWith => _$SaveOutcome_NeedsConfirmationCopyWithImpl<SaveOutcome_NeedsConfirmation>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SaveOutcome_NeedsConfirmation&&(identical(other.prepared, prepared) || other.prepared == prepared));
}


@override
int get hashCode => Object.hash(runtimeType,prepared);

@override
String toString() {
  return 'SaveOutcome.needsConfirmation(prepared: $prepared)';
}


}

/// @nodoc
abstract mixin class $SaveOutcome_NeedsConfirmationCopyWith<$Res> implements $SaveOutcomeCopyWith<$Res> {
  factory $SaveOutcome_NeedsConfirmationCopyWith(SaveOutcome_NeedsConfirmation value, $Res Function(SaveOutcome_NeedsConfirmation) _then) = _$SaveOutcome_NeedsConfirmationCopyWithImpl;
@useResult
$Res call({
 PreparedPulse prepared
});




}
/// @nodoc
class _$SaveOutcome_NeedsConfirmationCopyWithImpl<$Res>
    implements $SaveOutcome_NeedsConfirmationCopyWith<$Res> {
  _$SaveOutcome_NeedsConfirmationCopyWithImpl(this._self, this._then);

  final SaveOutcome_NeedsConfirmation _self;
  final $Res Function(SaveOutcome_NeedsConfirmation) _then;

/// Create a copy of SaveOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? prepared = null,}) {
  return _then(SaveOutcome_NeedsConfirmation(
prepared: null == prepared ? _self.prepared : prepared // ignore: cast_nullable_to_non_nullable
as PreparedPulse,
  ));
}


}

/// @nodoc


class SaveOutcome_Saved extends SaveOutcome {
  const SaveOutcome_Saved({required this.receipt}): super._();
  

 final  PulseReceipt receipt;

/// Create a copy of SaveOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SaveOutcome_SavedCopyWith<SaveOutcome_Saved> get copyWith => _$SaveOutcome_SavedCopyWithImpl<SaveOutcome_Saved>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SaveOutcome_Saved&&(identical(other.receipt, receipt) || other.receipt == receipt));
}


@override
int get hashCode => Object.hash(runtimeType,receipt);

@override
String toString() {
  return 'SaveOutcome.saved(receipt: $receipt)';
}


}

/// @nodoc
abstract mixin class $SaveOutcome_SavedCopyWith<$Res> implements $SaveOutcomeCopyWith<$Res> {
  factory $SaveOutcome_SavedCopyWith(SaveOutcome_Saved value, $Res Function(SaveOutcome_Saved) _then) = _$SaveOutcome_SavedCopyWithImpl;
@useResult
$Res call({
 PulseReceipt receipt
});




}
/// @nodoc
class _$SaveOutcome_SavedCopyWithImpl<$Res>
    implements $SaveOutcome_SavedCopyWith<$Res> {
  _$SaveOutcome_SavedCopyWithImpl(this._self, this._then);

  final SaveOutcome_Saved _self;
  final $Res Function(SaveOutcome_Saved) _then;

/// Create a copy of SaveOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? receipt = null,}) {
  return _then(SaveOutcome_Saved(
receipt: null == receipt ? _self.receipt : receipt // ignore: cast_nullable_to_non_nullable
as PulseReceipt,
  ));
}


}

// dart format on
