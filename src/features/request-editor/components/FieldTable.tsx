import { Plus, Trash2 } from "lucide-react";

import type { OrderedFieldDto } from "../../../shared/api/generated/ipc";
import { createEmptyField, normalizeFieldOrders, sortOrderedFields } from "../ordered-fields";
import { useI18n } from "../../../app/i18n";

type FieldTableProps = {
  fields: OrderedFieldDto[];
  legend: string;
  onChange: (fields: OrderedFieldDto[]) => void;
};

export function FieldTable({ fields, legend, onChange }: FieldTableProps) {
  const { t } = useI18n();
  const orderedFields = sortOrderedFields(fields);

  function updateField(
    index: number,
    updater: (field: OrderedFieldDto) => OrderedFieldDto,
  ) {
    const nextFields = orderedFields.map((field, fieldIndex) =>
      fieldIndex === index ? updater(field) : field,
    );
    onChange(normalizeFieldOrders(nextFields));
  }

  return (
    <fieldset className="min-h-0 rounded-md border border-slate-300 bg-white p-3">
      <div className="mb-3 flex items-center justify-between gap-3">
        <legend className="text-sm font-semibold text-slate-950">{legend}</legend>
        <button
          className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-2 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() =>
            onChange([
              ...orderedFields,
              createEmptyField(orderedFields.length),
            ])
          }
          type="button"
        >
          <Plus aria-hidden="true" size={14} />
          {t("fields.add")}
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[560px] table-fixed border-collapse text-sm">
          <thead>
            <tr className="border-y border-slate-200 bg-slate-50 text-left text-xs font-semibold uppercase text-slate-600">
              <th className="w-14 px-2 py-2">{t("fields.on")}</th>
              <th className="px-2 py-2">{t("fields.name")}</th>
              <th className="px-2 py-2">{t("fields.value")}</th>
              <th className="w-12 px-2 py-2">
                <span className="sr-only">{t("fields.actions")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {orderedFields.map((field, index) => (
              <tr className="border-b border-slate-200" key={field.order}>
                <td className="px-2 py-2">
                  <input
                    aria-label={t("fields.enabled", { legend, index: index + 1 })}
                    checked={field.enabled}
                    className="h-4 w-4 rounded border-slate-300 text-sky-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        enabled: event.currentTarget.checked,
                      }))
                    }
                    type="checkbox"
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} name`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        name: event.currentTarget.value,
                      }))
                    }
                    value={field.name}
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} value`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        value: event.currentTarget.value,
                      }))
                    }
                    value={field.value}
                  />
                </td>
                <td className="px-2 py-2">
                  <button
                    aria-label={t("fields.remove", { legend, index: index + 1 })}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onClick={() =>
                      onChange(
                        normalizeFieldOrders(
                          orderedFields.filter((_, fieldIndex) => fieldIndex !== index),
                        ),
                      )
                    }
                    type="button"
                  >
                    <Trash2 aria-hidden="true" size={15} />
                  </button>
                </td>
              </tr>
            ))}
            {orderedFields.length === 0 ? (
              <tr>
                <td className="px-2 py-5 text-center text-sm text-slate-500" colSpan={4}>
                  {t("fields.none", { legend: legend.toLowerCase() })}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </fieldset>
  );
}
