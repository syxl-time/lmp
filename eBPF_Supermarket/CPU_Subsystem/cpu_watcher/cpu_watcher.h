// Copyright 2023 The LMP Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://github.com/linuxkerneltravel/lmp/blob/develop/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// author: zhangziheng0525@163.com
//
// eBPF map for libbpf sar
#include <asm/types.h>
#include <linux/version.h>

typedef long long unsigned int u64;
typedef unsigned int u32;
typedef __kernel_mqd_t	mqd_t;
#define __user
#define MAX_CPU_NR	128
#define TASK_COMM_LEN 20
#define SYSCALL_MIN_TIME 1E7
#define MAX_SYSCALL_COUNT 100
#define MAX_ENTRIES 102400 // map容量

/*----------------------------------------------*/
/*          一些maps结构体的宏定义                */
/*----------------------------------------------*/
/// @brief 创建一个指定名字和键值类型的ebpf数组
/// @param name 新散列表的名字
/// @param type1 键的类型
/// @param type2 值的类型
/// @param MAX_ENTRIES map容量
#define BPF_ARRAY(name, type1,type2,MAX_ENTRIES )       \
	struct									\ 
	{										\
		__uint(type, BPF_MAP_TYPE_ARRAY);	\
		__uint(key_size, sizeof(type1));	\
		__uint(value_size, sizeof(type2));	\
		__uint(max_entries, MAX_ENTRIES);	\
	} name SEC(".maps")

/// @brief 创建一个指定名字和键值类型的ebpf散列表
/// @param name 新散列表的名字
/// @param type1 键的类型
/// @param type2 值的类型
/// @param MAX_ENTRIES 哈希map容量
#define BPF_HASH(name, type1,type2,MAX_ENTRIES )       \
    struct                                 \
    {                                      \
        __uint(type, BPF_MAP_TYPE_HASH);   \
        __uint(key_size, sizeof(type1));   \
        __uint(value_size, sizeof(type2)); \
        __uint(max_entries, MAX_ENTRIES);  \
    } name SEC(".maps")

/// @brief 创建一个指定名字和键值类型的ebpf每CPU数组
/// @param name 新散列表的名字
/// @param type1 键的类型
/// @param type2 值的类型
/// @param MAX_ENTRIES map容量
#define BPF_PERCPU_ARRAY(name, type1,type2,MAX_ENTRIES )       \
    struct                                 \
    {                                      \
        __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);   \
        __uint(key_size, sizeof(type1));   \
        __uint(value_size, sizeof(type2)); \
        __uint(max_entries, MAX_ENTRIES);  \
    } name SEC(".maps")

/// @brief 创建一个指定名字和键值类型的ebpf每CPU散列表
/// @param name 新散列表的名字
/// @param type1 键的类型
/// @param type2 值的类型
/// @param MAX_ENTRIES map容量
#define BPF_PERCPU_HASH(name, type1,type2,MAX_ENTRIES )       \
    struct                                 \
    {                                      \
        __uint(type, BPF_MAP_TYPE_PERCPU_HASH);   \
        __uint(key_size, sizeof(type1));   \
        __uint(value_size, sizeof(type2)); \
        __uint(max_entries, MAX_ENTRIES);  \
    } name SEC(".maps")

/*----------------------------------------------*/
/*          cs_delay结构体                     */
/*----------------------------------------------*/
#ifndef __CS_DELAY_H
#define __CS_DELAY_H
struct event {
	long unsigned int t1;
	long unsigned int t2;
	long unsigned int delay;   
};
#endif /* __CS_DELAY_H */

/*----------------------------------------------*/
/*          syscall_delay结构体                     */
/*----------------------------------------------*/
struct syscall_flags{
	long unsigned int start_time;
	int syscall_id;
};

struct syscall_events {//每个进程一个
    int pid,count;
    char comm[TASK_COMM_LEN];
    u64 delay;
    u64 syscall_id;
};
/*----------------------------------------------*/
/*         preempt_event结构体                     */
/*----------------------------------------------*/
struct preempt_event{
	pid_t prev_pid;
	pid_t next_pid;
	unsigned long long duration;
	char comm[TASK_COMM_LEN];
};
/*----------------------------------------------*/
/*         schedule_delay相关结构体                     */
/*----------------------------------------------*/
//标识不同进程
struct proc_id{
	int pid;
	int cpu_id;
}; 
//标识该进程的调度信息
struct schedule_event{
	int pid;
	int count;//调度次数
	unsigned long long enter_time;
};
//整个系统所有调度信息
struct sum_schedule{
	unsigned long long sum_count;
	unsigned long long sum_delay;
	unsigned long long max_delay;
	unsigned long long min_delay;
};

/*----------------------------------------------*/
/*         mq_delay相关结构体                     */
/*----------------------------------------------*/
struct mq_events {
    int send_pid;
    int rcv_pid;
    mqd_t mqdes;
        size_t msg_len;
        unsigned int msg_prio;
        
        u64 send_enter_time;
        u64 send_exit_time;
        u64 send_delay;

        u64 rcv_enter_time;
        u64 rcv_exit_time;
        u64 rcv_delay;
        u64 delay;
};
struct send_events {
    int send_pid;
    u64 Key_msg_ptr;
    
    mqd_t mqdes;
        size_t msg_len;
        unsigned int msg_prio;
        const char *u_msg_ptr;
        const void  *src;
        u64 send_enter_time;
        u64 send_exit_time;
};
struct rcv_events {
    int rcv_pid;
    u64 Key_msg_ptr;
    mqd_t mqdes;
        size_t msg_len;
        unsigned int msg_prio; 
        const char *u_msg_ptr;
        const void  *dest;
        u64 rcv_enter_time;
        u64 rcv_exit_time;
};
/*----------------------------------------------*/
/*          cswch_args结构体                     */
/*----------------------------------------------*/
struct cswch_args {
	u64 pad;
	char prev_comm[16];
	pid_t prev_pid;
	int prev_prio;
	long prev_state;
	char next_comm[16];
	pid_t next_pid;
	int next_prio;
};

/*----------------------------------------------*/
/*          软中断结构体                         */
/*----------------------------------------------*/
struct __softirq_info {
	u64 pad;
	u32 vec;
};

/*----------------------------------------------*/
/*          硬中断结构体                         */
/*----------------------------------------------*/
struct __irq_info {
	u64 pad;
	u32 irq;
};

/*----------------------------------------------*/
/*          idlecpu空闲时间所需结构体             */
/*----------------------------------------------*/
struct idleStruct {
	u64 pad;
	unsigned int state;
	unsigned int cpu_id;
};