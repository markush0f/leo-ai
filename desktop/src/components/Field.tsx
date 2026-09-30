import { useId, useState, type InputHTMLAttributes, type TextareaHTMLAttributes, type SelectHTMLAttributes, type ReactNode } from "react";
import { IconEye, IconEyeOff } from "../icons";
import { control, controlArea, controlInput, controlInvalid, cx, field, fieldAction, fieldHint, fieldIcon, fieldInvalid, focusTrack } from "../ui";

type FieldProps = { label: string; hint?: string; error?: string; icon?: ReactNode; className?: string };

function FieldShell({ id, label, hint, error, icon, className = "", children }: FieldProps & { id: string; children: ReactNode }) {
  return <div className={cx(field, error && fieldInvalid, className)}>
    <label htmlFor={id}>{label}</label>
    <div className={cx(control, icon ? "has-icon" : false, error && controlInvalid)}>
      {icon && <span className={fieldIcon}>{icon}</span>}
      {children}
      <span className={focusTrack} aria-hidden="true" />
    </div>
    {(error || hint) && <small className={fieldHint} id={`${id}-help`} role={error ? "alert" : undefined}>{error || hint}</small>}
  </div>;
}

export function Input({ label, hint, error, icon, className, id: givenId, type = "text", ...props }: FieldProps & InputHTMLAttributes<HTMLInputElement>) {
  const generatedId = useId();
  const id = givenId ?? generatedId;
  const [visible, setVisible] = useState(false);
  return <FieldShell {...{ id, label, hint, error, icon, className }}>
    <input {...props} id={id} type={type === "password" && visible ? "text" : type}
      className={cx(controlInput, icon ? "pl-[2.65rem]" : false, type === "password" && "pr-[2.8rem]")}
      aria-invalid={error ? true : props["aria-invalid"]}
      aria-describedby={[props["aria-describedby"], (hint || error) && `${id}-help`].filter(Boolean).join(" ") || undefined} />
    {type === "password" && <button type="button" className={fieldAction} disabled={props.disabled}
      aria-label={visible ? "Ocultar contraseña" : "Mostrar contraseña"} aria-pressed={visible}
      onMouseDown={(event) => event.preventDefault()} onClick={() => setVisible(!visible)}>
      {visible ? <IconEyeOff /> : <IconEye />}
    </button>}
  </FieldShell>;
}

export function TextArea({ label, hint, error, icon, className, id: givenId, ...props }: FieldProps & TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const generatedId = useId();
  const id = givenId ?? generatedId;
  return <FieldShell {...{ id, label, hint, error, icon, className }}>
    <textarea {...props} className={cx(controlInput, controlArea, icon ? "pl-[2.65rem]" : false)} id={id} aria-invalid={error ? true : props["aria-invalid"]} aria-describedby={(hint || error) ? `${id}-help` : props["aria-describedby"]} />
  </FieldShell>;
}

export function Select({ label, hint, error, icon, className, id: givenId, children, ...props }: FieldProps & SelectHTMLAttributes<HTMLSelectElement>) {
  const generatedId = useId();
  const id = givenId ?? generatedId;
  return <FieldShell {...{ id, label, hint, error, icon, className }}>
    <select {...props} className={cx(controlInput, icon ? "pl-[2.65rem]" : false)} id={id} aria-invalid={error ? true : props["aria-invalid"]} aria-describedby={(hint || error) ? `${id}-help` : props["aria-describedby"]}>{children}</select>
  </FieldShell>;
}
