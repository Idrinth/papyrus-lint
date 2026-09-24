export interface TypeNameRef {
  name: string;
  is_array: boolean;
}

export interface ParamRef {
  name: string;
  type_name: TypeNameRef;
}

export interface FunctionMember {
  kind: "function";
  name: string;
  params: ParamRef[];
  return_type: TypeNameRef | null;
  is_global: boolean;
  is_native: boolean;
  is_event: boolean;
  // Inner text of the `{ ... }` documentation comment after this function's
  // header, when the backend (or a local overlay of the buffer being edited)
  // found one. Absent/null/empty means there's nothing to show.
  doc?: string | null;
}

export interface PropertyMember {
  kind: "property";
  name: string;
  type_name: TypeNameRef;
  // Inner text of the `{ ... }` documentation comment after this property's
  // header; same rules as [`FunctionMember.doc`].
  doc?: string | null;
}

// Mirrors `papyrus_lint_core::function_table::Member`, as returned by the
// `list_script_members` Tauri command.
export type Member = FunctionMember | PropertyMember;

export interface IdentifierAt {
  name: string;
  start: number;
  end: number;
  receiver: string | null;
}
