import { useId, useState, type InputHTMLAttributes, type TextareaHTMLAttributes, type SelectHTMLAttributes, type ReactNode } from "react";
import { IconEye, IconEyeOff } from "../icons";

type FieldProps = { label: string; hint?: string; error?: string; icon?: ReactNode; className?: string };

function FieldShell({ id, label, hint, error, icon, className = "", children }: FieldProps & { id: string; children: ReactNode }) {
  return <div className={`ira-field ${error ? "invalid" : ""} ${className}`}>
    <label htmlFor={id}>{label}</label>
    <div className={`ira-control${icon ? " has-icon" : ""}`}>
      {icon && <span className="field-icon">{icon}</span>}
      {children}
      <span className="field-focus-track" aria-hidden="true" />
    </div>
    {(error || hint) && <small id={`${id}-help`} role={error ? "alert" : undefined}>{error || hint}</small>}
  </div>;
}

export function Input({ label, hint, error, icon, className, id: givenId, type = "text", ...props }: FieldProps & InputHTMLAttributes<HTMLInputElement>) {
  const generatedId = useId();
  const id = givenId ?? generatedId;
  const [visible, setVisible] = useState(false);
  return <FieldShell {...{ id, label, hint, error, icon, className }}>
    <input {...props} id={id} type={type === "password" && visible ? "text" : type}
      className={type === "password" ? "has-action" : undefined}
      aria-invalid={error ? true : props["aria-invalid"]}
      aria-describedby={[props["aria-describedby"], (hint || error) && `${id}-help`].filter(Boolean).join(" ") || undefined} />
    {type === "password" && <button type="button" className="field-action" disabled={props.disabled}
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
    <textarea {...props} id={id} aria-invalid={error ? true : props["aria-invalid"]} aria-describedby={(hint || error) ? `${id}-help` : props["aria-describedby"]} />
  </FieldShell>;
}

export function Select({ label, hint, error, icon, className, id: givenId, children, ...props }: FieldProps & SelectHTMLAttributes<HTMLSelectElement>) {
  const generatedId = useId();
  const id = givenId ?? generatedId;
  return <FieldShell {...{ id, label, hint, error, icon, className }}>
    <select {...props} id={id} aria-invalid={error ? true : props["aria-invalid"]} aria-describedby={(hint || error) ? `${id}-help` : props["aria-describedby"]}>{children}</select>
  </FieldShell>;
}
