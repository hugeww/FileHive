import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import type { ReceivedFileRecord, SentFileRecord } from '@/types'
import { STORAGE_KEYS } from '@/constants'
import { readStoredRecords, writeStoredRecords } from '@/utils/record-storage'

export const useFileDataStore = defineStore('fileData', () => {
  const receiveData = ref<ReceivedFileRecord[]>(
    readStoredRecords<ReceivedFileRecord>(STORAGE_KEYS.RECEIVE_RECORDS)
  )
  const shareData = ref<SentFileRecord[]>(
    readStoredRecords<SentFileRecord>(STORAGE_KEYS.SHARE_RECORDS)
  )

  watch(
    receiveData,
    (records) => {
      writeStoredRecords(STORAGE_KEYS.RECEIVE_RECORDS, records)
    },
    { deep: true }
  )

  watch(
    shareData,
    (records) => {
      writeStoredRecords(STORAGE_KEYS.SHARE_RECORDS, records)
    },
    { deep: true }
  )

  const addReceiveData = (record: ReceivedFileRecord) => {
    receiveData.value.push(record)
  }

  const removeReceiveData = (id: number) => {
    const index = receiveData.value.findIndex((record) => record.id === id)
    if (index !== -1) {
      receiveData.value.splice(index, 1)
    }
  }

  const deleteReceiveData = (index: number) => {
    if (index >= 0 && index < receiveData.value.length) {
      receiveData.value.splice(index, 1)
    }
  }

  const clearReceiveData = () => {
    receiveData.value = []
  }

  const addShareDataRecord = (record: SentFileRecord) => {
    shareData.value.push(record)
  }

  const deleteShareData = (index: number) => {
    if (index >= 0 && index < shareData.value.length) {
      shareData.value.splice(index, 1)
    }
  }

  const clearShareData = () => {
    shareData.value = []
  }

  return {
    receiveData,
    shareData,
    addReceiveData,
    removeReceiveData,
    deleteReceiveData,
    clearReceiveData,
    addShareDataRecord,
    deleteShareData,
    clearShareData
  }
})
