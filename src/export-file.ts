export function exportJson(value: unknown, name: string) {
  exportText(JSON.stringify(value, null, 2), name);
}
export function exportText(value: string, name: string) {
  const url = URL.createObjectURL(
    new Blob([value], {
      type: "application/json;charset=utf-8",
    }),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
