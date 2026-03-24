/**
 * Sample TypeScript file exercising all extractor features.
 */

import { EventEmitter } from "events";
import * as path from "path";

export const MAX_RETRIES = 3;

export type UserId = string;

/** Represents a user in the system. */
export interface IUser {
    readonly id: UserId;
    name: string;
    getDisplayName(): string;
}

export enum Role {
    Admin = "ADMIN",
    User = "USER",
    Guest = "GUEST",
}

function log(message: string): void {
    console.log(message);
}

/** Decorator that logs method calls. */
function LogMethod(target: any, key: string, descriptor: PropertyDescriptor) {
    return descriptor;
}

@LogMethod
export class UserService extends EventEmitter implements IUser {
    readonly id: UserId;
    name: string;
    private _cache: Map<string, unknown> = new Map();
    protected settings: Record<string, string> = {};

    constructor(id: UserId, name: string) {
        super();
        this.id = id;
        this.name = name;
    }

    getDisplayName(): string {
        return `${this.name} (${this.id})`;
    }

    @LogMethod
    async fetchProfile(url: string): Promise<Record<string, unknown>> {
        const response = await fetch(url);
        const data = await response.json();
        this._cache.set(url, data);
        log(`Fetched profile for ${this.name}`);
        return data as Record<string, unknown>;
    }

    private resetCache(): void {
        this._cache.clear();
    }
}

export const createUser = (id: string, name: string): UserService => {
    return new UserService(id, name);
};

export namespace Auth {
    export function validate(token: string): boolean {
        return token.length > 0;
    }

    export class TokenManager {
        private tokens: string[] = [];

        addToken(token: string): void {
            this.tokens.push(token);
        }
    }
}
