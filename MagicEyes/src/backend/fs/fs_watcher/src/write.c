#include <stdio.h>
#include <signal.h>
#include <unistd.h>
#include <sys/resource.h>
#include <time.h>
#include <bpf/libbpf.h>

#include "fs_watcher/include/write.h"
#include "fs/fs_watcher/write.skel.h"

static int libbpf_print_fn(enum libbpf_print_level level, const char *format, va_list args)
{
	return vfprintf(stderr, format, args);
}

static volatile bool exiting = false;

static void sig_handler(int sig)
{
	exiting = true;
}

static int write_event(void *ctx, void *data, size_t data_sz)
{
    const struct fs_t *e = data;
    struct tm *tm;
    char ts[32];
    time_t t;

    time(&t);
    tm = localtime(&t);
    strftime(ts, sizeof(ts), "%H:%M:%S", tm);
    printf("%-8s  %-7d  %-7llu\n", ts, e->pid,e->duration_ns);
    return 0;
}

int main(int argc, char **argv)
{
    struct ring_buffer *rb = NULL;
    struct write_bpf *skel;
	int err;
   
    /* Set up libbpf errors and debug info callback */
	libbpf_set_print(libbpf_print_fn);

	
	/* Cleaner handling of Ctrl-C */
	signal(SIGINT, sig_handler);
	signal(SIGTERM, sig_handler);

    /* Open BPF application */
	skel = write_bpf__open();
	if (!skel) {
		fprintf(stderr, "Failed to open BPF skeleton\n");
		return 1;
	}
    
	/* Load & verify BPF programs */
	err = write_bpf__load(skel);
	if (err) {
		fprintf(stderr, "Failed to load and verify BPF skeleton\n");
		goto cleanup;
	}

    /* Attach tracepoints */
	err = write_bpf__attach(skel);
	if (err) {
		fprintf(stderr, "Failed to attach BPF skeleton\n");
		goto cleanup;
	}

    /* Set up ring buffer polling */
	rb = ring_buffer__new(bpf_map__fd(skel->maps.rb), write_event, NULL, NULL);
	if (!rb) {
		err = -1;
		fprintf(stderr, "Failed to create ring buffer\n");
		goto cleanup;
	}

    /* Process events */
	printf("%-8s  %-7s   %-7s\n", "TIME", "PID", "durations");
	while (!exiting) {
		err = ring_buffer__poll(rb, 100 /* timeout, ms */);
		/* Ctrl-C will cause -EINTR */
		if (err == -EINTR) {
			err = 0;
			break;
		}

		if (err < 0) {
			printf("Error polling perf buffer: %d\n", err);
			break;
		}
	}

cleanup:
	/* Clean up */
	ring_buffer__free(rb);
	write_bpf__destroy(skel);

	return err < 0 ? -err : 0;
}
