export default function Hexdump({ data }: { data: Uint8Array }) {
  const rows = [];
  for (let i = 0; i < data.length; i += 16) {
    const row = [];
    const chars = [];
    for (let j = 0; j < 16; j++) {
      const value = data[i + j];
      row.push(value.toString(16).padStart(2, "0"));
      chars.push(
        value >= 32 && value <= 126 ? String.fromCharCode(value) : "."
      );
    }
    rows.push(row.join(" ") + "  " + chars.join(""));
  }
  return (
    <pre>
      {rows.map((row, i) => (
        <div key={i}>{row}</div>
      ))}
    </pre>
  );
}
