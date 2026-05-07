// React contexts shared by the form hook and the atomic field components.
// Lives in its own file to break the form-hook ↔ form-fields import cycle.

import { createFormHookContexts } from "@tanstack/react-form";

export const { fieldContext, formContext, useFieldContext, useFormContext } =
  createFormHookContexts();
