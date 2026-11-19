import { useCallback, useEffect, useState } from 'react'

import { ApiError, checkOwnerToken } from '../api/client'

const STORAGE_KEY = 'tickstream.ownerToken'

export interface Owner {
  /** Token pemilik, atau `null` untuk pengunjung (cuma bisa melihat). */
  token: string | null
  /** Cek token ke server dulu; disimpan cuma kalau server menerimanya. */
  signIn: (token: string) => Promise<void>
  signOut: () => void
}

/**
 * Mode pemilik. Token disimpan di localStorage browser pemilik sendiri, jadi cukup
 * dimasukkan sekali per perangkat. localStorage bisa nggak tersedia (mode privat, cookie
 * diblokir); tanpa itu, token cuma bertahan sampai halaman ditutup.
 */
export function useOwner(): Owner {
  const [token, setToken] = useState<string | null>(readToken)

  // Token tersimpan bisa sudah nggak berlaku (diganti di server). Cek sekali saat halaman
  // dibuka, supaya tombol tambah/hapus nggak muncul kalau ujung-ujungnya ditolak. Token
  // yang masuk lewat signIn nggak perlu dicek di sini: signIn sudah mengeceknya.
  useEffect(() => {
    const saved = readToken()
    if (!saved) return
    const controller = new AbortController()
    checkOwnerToken(saved, controller.signal).catch((err: unknown) => {
      if (isUnauthorized(err)) {
        writeToken(null)
        setToken(null)
      }
    })
    return () => controller.abort()
  }, [])

  const signIn = useCallback(async (candidate: string) => {
    const trimmed = candidate.trim()
    await checkOwnerToken(trimmed)
    writeToken(trimmed)
    setToken(trimmed)
  }, [])

  const signOut = useCallback(() => {
    writeToken(null)
    setToken(null)
  }, [])

  return { token, signIn, signOut }
}

/** 401 dari server: token nggak ada atau salah. */
export function isUnauthorized(err: unknown): boolean {
  return err instanceof ApiError && err.status === 401
}

function readToken(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

function writeToken(token: string | null) {
  try {
    if (token) localStorage.setItem(STORAGE_KEY, token)
    else localStorage.removeItem(STORAGE_KEY)
  } catch {
    // Penyimpanan diblokir: mode pemilik tetap jalan untuk sesi ini.
  }
}
