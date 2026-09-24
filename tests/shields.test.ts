import { describe, expect, it } from 'vitest';
import {
  domainFromPattern,
  domainPattern,
  fromStored,
  resolvePattern,
  toStored,
} from '../src/shields.js';
import { PortError } from '../src/errors.js';

describe('shields toStored', () => {
  it('shields values', () => {
    let entries = toStored('shields', 'on', '0');
    expect(entries).toHaveLength(1);
    expect(entries[0]![0]).toBe('braveShields');
    expect((entries[0]![1] as Record<string, unknown>)['setting']).toBe(1);

    entries = toStored('shields', 'off', '0');
    expect((entries[0]![1] as Record<string, unknown>)['setting']).toBe(2);
  });

  it('fingerprinting values', () => {
    expect(
      (toStored('fingerprinting', 'standard', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(3);
    expect(
      (toStored('fingerprinting', 'aggressive', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(2);
    expect(
      (toStored('fingerprinting', 'disabled', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(1);
  });

  it('https-upgrade values', () => {
    expect(
      (toStored('https-upgrade', 'standard', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(3);
    expect(
      (toStored('https-upgrade', 'strict', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(2);
    expect(
      (toStored('https-upgrade', 'disabled', '0')[0]![1] as Record<string, unknown>)[
        'setting'
      ],
    ).toBe(1);
  });

  it('scripts values', () => {
    expect(
      (toStored('scripts', 'allow', '0')[0]![1] as Record<string, unknown>)['setting'],
    ).toBe(1);
    expect(
      (toStored('scripts', 'block', '0')[0]![1] as Record<string, unknown>)['setting'],
    ).toBe(2);
  });

  it('ads triple-key writes', () => {
    let entries = toStored('ads', 'standard', '0');
    expect(entries).toHaveLength(3);
    expect(entries[0]![0]).toBe('shieldsAds');
    expect((entries[0]![1] as Record<string, unknown>)['setting']).toBe(2);
    expect(entries[1]![0]).toBe('trackers');
    expect((entries[1]![1] as Record<string, unknown>)['setting']).toBe(2);
    expect(entries[2]![0]).toBe('cosmeticFilteringV2');
    expect(
      (entries[2]![1] as Record<string, Record<string, unknown>>)['setting'][
        'cosmeticFilteringV2'
      ],
    ).toBe(2);

    entries = toStored('ads', 'aggressive', '0');
    expect((entries[0]![1] as Record<string, unknown>)['setting']).toBe(2);
    expect(
      (entries[2]![1] as Record<string, Record<string, unknown>>)['setting'][
        'cosmeticFilteringV2'
      ],
    ).toBe(1);

    entries = toStored('ads', 'disabled', '0');
    expect((entries[0]![1] as Record<string, unknown>)['setting']).toBe(1);
    expect((entries[1]![1] as Record<string, unknown>)['setting']).toBe(1);
    expect(
      (entries[2]![1] as Record<string, Record<string, unknown>>)['setting'][
        'cosmeticFilteringV2'
      ],
    ).toBe(0);
  });
});

describe('shields fromStored', () => {
  it('shields', () => {
    expect(fromStored('shields', 1)).toBe('on');
    expect(fromStored('shields', 2)).toBe('off');
  });

  it('fingerprinting', () => {
    expect(fromStored('fingerprinting', 3)).toBe('standard');
    expect(fromStored('fingerprinting', 2)).toBe('aggressive');
    expect(fromStored('fingerprinting', 1)).toBe('disabled');
  });

  it('https-upgrade', () => {
    expect(fromStored('https-upgrade', 3)).toBe('standard');
    expect(fromStored('https-upgrade', 2)).toBe('strict');
    expect(fromStored('https-upgrade', 1)).toBe('disabled');
  });

  it('scripts', () => {
    expect(fromStored('scripts', 1)).toBe('allow');
    expect(fromStored('scripts', 2)).toBe('block');
  });

  it('ads', () => {
    expect(fromStored('ads', 2, 2)).toBe('standard');
    expect(fromStored('ads', 2, 1)).toBe('aggressive');
    expect(fromStored('ads', 1, 0)).toBe('disabled');
  });
});

describe('patterns', () => {
  it('domain pattern', () => {
    expect(domainPattern('example.com')).toBe('example.com,*');
  });

  it('domain from pattern', () => {
    expect(domainFromPattern('example.com,*')).toBe('example.com');
    expect(domainFromPattern('invalid')).toBeNull();
  });

  it('unknown stored value errors', () => {
    expect(() => fromStored('shields', 99)).toThrow(PortError);
  });

  it('invalid cli value errors', () => {
    expect(() => toStored('shields', 'invalid', '0')).toThrow(PortError);
  });

  it('rejects quotes', () => {
    expect(() => domainPattern('foo"bar')).toThrow(PortError);
    expect(() => domainPattern("foo'bar")).toThrow(PortError);
  });

  it('rejects commas', () => {
    expect(() => domainPattern('foo,bar')).toThrow(PortError);
  });

  it('rejects unicode', () => {
    expect(() => domainPattern('exämple.com')).toThrow(PortError);
    expect(() => domainPattern('\u200bevil.com')).toThrow(PortError);
  });

  it('rejects empty', () => {
    expect(() => domainPattern('')).toThrow(PortError);
  });

  it('accepts valid domains', () => {
    expect(() => domainPattern('example.com')).not.toThrow();
    expect(() => domainPattern('sub.example.com')).not.toThrow();
    expect(() => domainPattern('my-site.co.uk')).not.toThrow();
    expect(() => domainPattern('test_site.com')).not.toThrow();
  });

  it('resolvePattern raw mode', () => {
    expect(resolvePattern('[example.com,*]', true)).toBe('[example.com,*]');
    expect(() => resolvePattern('', true)).toThrow(/pattern must not be empty/);
    expect(resolvePattern('example.com', false)).toBe('example.com,*');
  });
});