import { looksNumeric } from "../format";

/** The detail hero's key/value list: keys in dim, values in mono where they are figures (§10). */
export function Facts({ items }: { items: [string, string][] }) {
  if (items.length === 0) return null;
  return (
    <dl className="bs-facts">
      {items.map(([key, value], i) => (
        // A key can repeat ("Depends" twice), so the index is part of the identity.
        <FactRow key={`${key}-${i}`} name={key} value={value} />
      ))}
    </dl>
  );
}

function FactRow({ name, value }: { name: string; value: string }) {
  return (
    <>
      <dt>{name}</dt>
      <dd className={looksNumeric(value) ? "n" : undefined}>{value}</dd>
    </>
  );
}
