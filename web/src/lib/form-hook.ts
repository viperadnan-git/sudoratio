// App-wide TanStack Form hook bound to shadcn field atoms.
//
// Use `useAppForm` instead of `useForm` at the form root. Inside it,
// `<form.AppField name="x"><Atom .../></form.AppField>` renders a child whose
// `useFieldContext` resolves to the named field — the atom needs no props for
// value/handler wiring.
//
// Reusable sub-forms go through `withFieldGroup` (e.g. `PresetPolicyFields`).

import { createFormHook } from "@tanstack/react-form";

import {
  CheckboxRow,
  ClientProfileField,
  ColorPickerField,
  InlineEditField,
  NullableNumberRow,
  NumberInput,
  NumberRow,
} from "@/components/form-fields";
import { fieldContext, formContext } from "@/lib/form-contexts";

export const { useAppForm, withFieldGroup, withForm } = createFormHook({
  fieldComponents: {
    NumberRow,
    NullableNumberRow,
    NumberInput,
    CheckboxRow,
    InlineEditField,
    ColorPickerField,
    ClientProfileField,
  },
  formComponents: {},
  fieldContext,
  formContext,
});
