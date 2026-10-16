import type { AxiosRequestConfig } from 'axios'
import type { UploadProgress } from '@/types'

export const multipartUploadConfig = (
  onProgress?: (progress: UploadProgress) => void
): AxiosRequestConfig => ({
  timeout: 0,
  onUploadProgress: (progressEvent) => {
    if (onProgress && progressEvent.total) {
      onProgress({
        loaded: progressEvent.loaded,
        total: progressEvent.total,
        percentage: Math.round((progressEvent.loaded * 100) / progressEvent.total)
      })
    }
  }
})
