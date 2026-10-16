import { describe, expect, it } from "vitest";
import { describeWeather, fetchWeather, parseWeather, weatherKind } from "./weather";

const sample = {
  current: { temperature_2m: 31.4, apparent_temperature: 36.2, relative_humidity_2m: 62, wind_speed_10m: 11.5, weather_code: 2 },
  daily: { temperature_2m_max: [34.1], temperature_2m_min: [24.8] },
};

describe("clima de Mérida", () => {
  it("interpreta la respuesta de Open-Meteo", () => {
    expect(parseWeather(sample)).toEqual({ temperature: 31.4, feelsLike: 36.2, humidity: 62, wind: 11.5, code: 2, max: 34.1, min: 24.8 });
  });

  it("sin temperatura la respuesta no vale", () => {
    expect(parseWeather({})).toBeNull();
    expect(parseWeather({ current: { temperature_2m: "x" } })).toBeNull();
  });

  it("los datos que faltan quedan vacíos sin romper nada", () => {
    expect(parseWeather({ current: { temperature_2m: 20 } })).toMatchObject({ temperature: 20, feelsLike: null, max: null, code: -1 });
  });

  it("describe los códigos del tiempo", () => {
    expect(describeWeather(0)).toBe("Despejado");
    expect(describeWeather(63)).toBe("Lluvia");
    expect(describeWeather(95)).toBe("Tormenta");
    expect(describeWeather(-1)).toBe("Sin datos");
  });

  it("un fallo de red o una respuesta mala devuelve null", async () => {
    expect(await fetchWeather((async () => { throw new Error("sin red"); }) as unknown as typeof fetch)).toBeNull();
    expect(await fetchWeather((async () => ({ ok: false })) as unknown as typeof fetch)).toBeNull();
    expect(await fetchWeather((async () => ({ ok: true, json: async () => sample })) as unknown as typeof fetch)).toMatchObject({ temperature: 31.4 });
  });
});

describe("weatherKind", () => {
  it("elige el icono según el código y la hora", () => {
    expect(weatherKind(0)).toBe("sun");
    expect(weatherKind(0, true)).toBe("moon");
    expect(weatherKind(2)).toBe("partly-sun");
    expect(weatherKind(2, true)).toBe("partly-moon");
    expect(weatherKind(3)).toBe("cloud");
    expect(weatherKind(45)).toBe("fog");
    expect(weatherKind(53)).toBe("drizzle");
    expect(weatherKind(63)).toBe("rain");
    expect(weatherKind(81)).toBe("rain");
    expect(weatherKind(73)).toBe("snow");
    expect(weatherKind(95)).toBe("storm");
    expect(weatherKind(999)).toBe("unknown");
  });
});
